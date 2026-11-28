//! The half that holds the control plane.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use blob::Tenant;
use clock::Clock;
use control::{Control, DomainName, Handle, Id, Ports, Site, SiteKind, Slug};
use edge::{Deploy, Hostname, Notice, Resolved};
use edge_folder::Placed;
use jiff::{SignedDuration, Timestamp};
use logging::Field as _;
use model::{ContentPath, DeployId};
use tokio::sync::mpsc;

use crate::lookup::cache::Recent;
use crate::lookup::caller::{Answer, Ask, Call, Noticed};
use crate::{Chain, Failure, Fronts, Locate, Settings};

/// The deploy a handle serves.
#[derive(Clone, Copy, Debug)]
struct Pointer {
    site: Id<SiteKind>,
    deploy: DeployId,
    at: Timestamp,
}

/// A deploy read and placed, shared by every request for it.
#[derive(Clone, Debug)]
struct Loaded {
    deploy: Arc<Deploy>,
    placed: Arc<Placed>,
}

/// Answers lookups one at a time, from the control plane and what it kept of earlier ones.
#[derive(Debug)]
pub struct Back<P: Ports, K, L> {
    calls: mpsc::Receiver<Call>,
    control: Control<P>,
    clock: K,
    locate: L,
    ttl: std::time::Duration,
    pointers: RefCell<Recent<Handle, Pointer>>,
    deploys: RefCell<Recent<(Id<SiteKind>, DeployId), Loaded>>,
}

impl<P: Ports + 'static, K: Clock, L: Locate> Back<P, K, L> {
    /// The two halves over `control`, whose blobs `locate` says where to find and whose time is
    /// `clock`'s.
    pub fn pair(control: Control<P>, clock: K, locate: L, settings: Settings) -> (Fronts, Self) {
        let (calls, received) = mpsc::channel(settings.waiting.max(1));
        let back = Self {
            calls: received,
            control,
            clock,
            locate,
            ttl: settings.ttl,
            pointers: RefCell::new(Recent::new(settings.deploys)),
            deploys: RefCell::new(Recent::new(settings.deploys)),
        };
        (Fronts::new(calls), back)
    }

    /// Answers lookups until every [`Fronts`] is dropped.
    pub async fn run(mut self) {
        logging::info!(name: logging::event::SITES_SERVING, ttl_us = logging::Took(self.ttl).field());
        let mut asked = 0_u64;
        while let Some(Call { hostname, ask }) = self.calls.recv().await {
            asked += 1;
            match ask {
                Ask::Serve(reply) => {
                    let answer = self.answer(&hostname).await;
                    if let Err(failure) = &answer {
                        logging::warn!(name: logging::event::SITES_FAILED, error = logging::Failure(failure).field());
                    }
                    // a caller that went away no longer wants its answer
                    reply.send(answer).ok();
                }
                Ask::Suspended(reply) => {
                    let answer = self.notice(&hostname).await;
                    if let Err(failure) = &answer {
                        logging::warn!(name: logging::event::SITES_FAILED, error = logging::Failure(failure).field());
                    }
                    reply.send(answer).ok();
                }
            }
        }
        logging::info!(name: logging::event::SITES_STOPPED, asked = logging::Count(asked).field());
    }

    /// What `hostname` serves now.
    async fn answer(&self, hostname: &Hostname) -> Answer {
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
        let loaded = self.loaded(&pointer).await?;
        Ok(Some(Resolved { deploy: loaded.deploy, scope: loaded.placed }))
    }

    /// The notice of `hostname`'s site, when the site is suspended. Never remembered: a
    /// suspension is asked of the control plane each time, and only for a host that serves
    /// nothing.
    async fn notice(&self, hostname: &Hostname) -> Noticed {
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

    /// The deploy of a pointer, its manifest read back and its blobs placed in the store's files.
    async fn loaded(&self, pointer: &Pointer) -> Result<Loaded, Failure> {
        let key = (pointer.site, pointer.deploy);
        let kept = self.deploys.borrow().get(&key).cloned();
        if let Some(loaded) = kept {
            return Ok(loaded);
        }
        let lookup = |error: &(dyn std::error::Error + 'static)| Failure::Lookup(Chain::of(error));
        let manifest = self.control.manifest(pointer.site, pointer.deploy).await.map_err(|error| lookup(&error))?;
        let tenant = Tenant::try_from(pointer.site).map_err(|error| lookup(&error))?;
        let mut files = BTreeMap::new();
        for entry in manifest.entries() {
            let path = ContentPath::parse(entry.blob().to_string()).map_err(|error| lookup(&error))?;
            files.insert(entry.blob(), path);
        }
        let count = logging::Count::from(files.len());
        let placed = Arc::new(Placed::new(self.locate.folder(&tenant), files));
        let deploy = Arc::new(Deploy { manifest, ..Deploy::default() });
        let loaded = Loaded { deploy, placed };
        self.deploys.borrow_mut().put(key, loaded.clone());
        logging::debug!(
            name: logging::event::SITES_PLACED,
            site = logging::Shown(pointer.site).field(),
            deploy = logging::Shown(pointer.deploy).field(),
            files = count.field(),
        );
        Ok(loaded)
    }
}
