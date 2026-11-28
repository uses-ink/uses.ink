//! The lookup itself: a host to the deploy it serves, over a control plane and a clock.
//!
//! This is the part of a lookup that does not depend on where the blobs are. [`crate::Back`] adds
//! the placing of a deploy's blobs as files; a Worker adds nothing and reads them by hash from
//! the blob store.

use std::cell::RefCell;
use std::sync::Arc;

use blob::Tenant;
use clock::Clock;
use control::{Control, DomainName, Handle, Id, Ports, Site, SiteKind, Slug};
use edge::{Deploy, Hostname, Notice};
use jiff::{SignedDuration, Timestamp};
use logging::Field as _;
use model::DeployId;

use crate::lookup::cache::{Recent, Settings};
use crate::{Chain, Failure};

/// What a lookup of a suspension answers.
pub(crate) type Noticed = Result<Option<Notice>, Failure>;

/// The deploy a handle serves.
#[derive(Clone, Copy, Debug)]
struct Pointer {
    site: Id<SiteKind>,
    deploy: DeployId,
    at: Timestamp,
}

/// What a host serves: the deploy with its manifest, and the tenant its blobs are read under.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// The site.
    pub site: Id<SiteKind>,
    /// The deploy, shared by every request for it.
    pub deploy: Arc<Deploy>,
    /// Whose blobs the deploy's are.
    pub tenant: Tenant,
}

/// Answers lookups from the control plane and what it kept of earlier ones. It is not `Sync`: it
/// is held by one task, or by a Worker's single thread.
#[derive(Debug)]
pub struct Found<P: Ports, K> {
    control: Control<P>,
    clock: K,
    ttl: std::time::Duration,
    pointers: RefCell<Recent<Handle, Pointer>>,
    deploys: RefCell<Recent<(Id<SiteKind>, DeployId), Loaded>>,
}

impl<P: Ports + 'static, K: Clock> Found<P, K> {
    /// A lookup over `control`, whose time is `clock`'s, kept as `settings` say.
    pub fn new(control: Control<P>, clock: K, settings: &Settings) -> Self {
        Self {
            control,
            clock,
            ttl: settings.ttl,
            pointers: RefCell::new(Recent::new(settings.deploys)),
            deploys: RefCell::new(Recent::new(settings.deploys)),
        }
    }

    /// How long a handle's live deploy is remembered.
    pub const fn ttl(&self) -> std::time::Duration {
        self.ttl
    }

    /// The control plane the lookup reads.
    pub const fn control(&self) -> &Control<P> {
        &self.control
    }

    /// What `hostname` serves now.
    pub async fn resolve(&self, hostname: &Hostname) -> Result<Option<Loaded>, Failure> {
        let Some(name) = hostname.name() else { return Ok(None) };
        let origin = self.control.origin();
        let pointer = match (origin.handle(name), origin.preview(name)) {
            (Some(handle), _) => self.pointer(&handle).await?,
            (None, Some((slug, handle))) => self.previewed(&slug, &handle).await?,
            (None, None) => match self.owner(name).await? {
                Some(site) => self.pointer(&site.handle).await?,
                None => return Ok(None),
            },
        };
        let Some(pointer) = pointer else { return Ok(None) };
        self.loaded(&pointer).await.map(Some)
    }

    /// The notice of `hostname`'s site, when the site is suspended. Never remembered: a
    /// suspension is asked of the control plane each time, and only for a host that serves
    /// nothing.
    pub async fn notice(&self, hostname: &Hostname) -> Noticed {
        let Some(name) = hostname.name() else { return Ok(None) };
        let handle = match self.control.origin().handle(name) {
            Some(handle) => handle,
            None => match self.owner(name).await? {
                Some(site) => site.handle,
                None => return Ok(None),
            },
        };
        let suspended = self.control.suspended(&handle).await.map_err(|error| Failure::Lookup(Chain::of(&error)))?;
        Ok(suspended.map(|suspension| Notice { reason: suspension.kind.name() }))
    }

    /// The site that proved `name` as a custom domain. Never remembered, like a preview: it is one
    /// indexed read, and a domain that was removed or whose site was removed stops serving at once.
    /// A host that is no domain name is no custom domain.
    async fn owner(&self, name: &str) -> Result<Option<Site>, Failure> {
        let Ok(name) = name.parse::<DomainName>() else { return Ok(None) };
        self.control.domained(&name).await.map_err(|error| Failure::Lookup(Chain::of(&error)))
    }

    /// Whether a pointer made at `at` is still believed at `now`.
    fn fresh(&self, at: Timestamp, now: Timestamp) -> bool {
        SignedDuration::try_from(self.ttl).is_ok_and(|ttl| now.duration_since(at) < ttl)
    }

    /// The deploy `handle` points at: remembered for the time to live, else asked of the control
    /// plane. A handle with no site, or a site with nothing live, has none.
    async fn pointer(&self, handle: &Handle) -> Result<Option<Pointer>, Failure> {
        let now = self.clock.now();
        let kept = self.pointers.borrow().get(handle).copied();
        if let Some(pointer) = kept.filter(|pointer| self.fresh(pointer.at, now)) {
            return Ok(Some(pointer));
        }
        let serving = self.control.serving(handle).await.map_err(|error| Failure::Lookup(Chain::of(&error)))?;
        logging::debug!(
            name: logging::event::SITES_ASKED,
            handle = logging::Shown(handle).field(),
            outcome = if serving.is_some() { "serving" } else { "none" },
        );
        let Some(serving) = serving else { return Ok(None) };
        let pointer = Pointer { site: serving.site, deploy: serving.live.deploy, at: now };
        self.pointers.borrow_mut().put(handle.clone(), pointer);
        Ok(Some(pointer))
    }

    /// The deploy `slug`'s preview of `handle`'s site serves, or none once it has ended. Never
    /// remembered: a preview lives for days and is asked of the control plane at each request, so
    /// the end of one and the removal of its site apply at once.
    async fn previewed(&self, slug: &Slug, handle: &Handle) -> Result<Option<Pointer>, Failure> {
        let now = self.clock.now();
        let showing =
            self.control.previewed((slug, handle), now).await.map_err(|error| Failure::Lookup(Chain::of(&error)))?;
        Ok(showing.map(|showing| Pointer { site: showing.site, deploy: showing.deploy, at: now }))
    }

    /// The deploy of a pointer, its manifest read back from the blob store.
    async fn loaded(&self, pointer: &Pointer) -> Result<Loaded, Failure> {
        let key = (pointer.site, pointer.deploy);
        let kept = self.deploys.borrow().get(&key).cloned();
        if let Some(loaded) = kept {
            return Ok(loaded);
        }
        let lookup = |error: &(dyn std::error::Error + 'static)| Failure::Lookup(Chain::of(error));
        let manifest = self.control.manifest(pointer.site, pointer.deploy).await.map_err(|error| lookup(&error))?;
        let tenant = Tenant::try_from(pointer.site).map_err(|error| lookup(&error))?;
        let deploy = Arc::new(Deploy { manifest, ..Deploy::default() });
        let loaded = Loaded { site: pointer.site, deploy, tenant };
        self.deploys.borrow_mut().put(key, loaded.clone());
        logging::debug!(
            name: logging::event::SITES_PLACED,
            site = logging::Shown(pointer.site).field(),
            deploy = logging::Shown(pointer.deploy).field(),
            files = logging::Count::from(loaded.deploy.manifest.entries().len()).field(),
        );
        Ok(loaded)
    }
}
