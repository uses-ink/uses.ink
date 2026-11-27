//! The half that holds the control plane.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use blob::Tenant;
use clock::Clock;
use control::{Control, Handle, Id, Ports, SiteKind};
use edge::{Deploy, Hostname, Resolved};
use edge_folder::Placed;
use jiff::{SignedDuration, Timestamp};
use model::{ContentPath, DeployId};
use tokio::sync::mpsc;

use crate::lookup::cache::Recent;
use crate::lookup::caller::{Answer, Call};
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
        while let Some(Call { hostname, reply }) = self.calls.recv().await {
            // a caller that went away no longer wants its answer
            reply.send(self.answer(&hostname).await).ok();
        }
    }

    /// What `hostname` serves now.
    async fn answer(&self, hostname: &Hostname) -> Answer {
        let Some(handle) = hostname.name().and_then(|name| self.control.origin().handle(name)) else {
            return Ok(None);
        };
        let Some(pointer) = self.pointer(&handle).await? else { return Ok(None) };
        let loaded = self.loaded(&pointer).await?;
        Ok(Some(Resolved { deploy: loaded.deploy, scope: loaded.placed }))
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
        let Some(serving) = serving else { return Ok(None) };
        let pointer = Pointer { site: serving.site, deploy: serving.live.deploy, at: now };
        self.pointers.borrow_mut().put(handle.clone(), pointer);
        Ok(Some(pointer))
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
        let placed = Arc::new(Placed::new(self.locate.folder(&tenant), files));
        let deploy = Arc::new(Deploy { manifest, ..Deploy::default() });
        let loaded = Loaded { deploy, placed };
        self.deploys.borrow_mut().put(key, loaded.clone());
        Ok(loaded)
    }
}
