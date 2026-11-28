//! The half that holds the control plane.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use blob::Tenant;
use clock::Clock;
use control::{Control, Id, Ports, SiteKind};
use edge::{Hostname, Resolved};
use edge_folder::Placed;
use logging::Field as _;
use model::{ContentPath, DeployId};
use tokio::sync::mpsc;

use crate::lookup::cache::Recent;
use crate::lookup::caller::{Answer, Ask, Call};
use crate::lookup::found::{Found, Loaded, Noticed};
use crate::{Chain, Failure, Fronts, Locate, Settings};

/// The places of the deploys kept, by site and deploy.
type Kept = RefCell<Recent<(Id<SiteKind>, DeployId), Arc<Placed>>>;

/// Answers lookups one at a time, from the control plane and what it kept of earlier ones.
#[derive(Debug)]
pub struct Back<P: Ports, K, L> {
    calls: mpsc::Receiver<Call>,
    found: Found<P, K>,
    locate: L,
    placed: Kept,
}

impl<P: Ports + 'static, K: Clock, L: Locate> Back<P, K, L> {
    /// The two halves over `control`, whose blobs `locate` says where to find and whose time is
    /// `clock`'s.
    pub fn pair(control: Control<P>, clock: K, locate: L, settings: Settings) -> (Fronts, Self) {
        let (calls, received) = mpsc::channel(settings.waiting.max(1));
        let back = Self {
            calls: received,
            found: Found::new(control, clock, &settings),
            locate,
            placed: RefCell::new(Recent::new(settings.deploys)),
        };
        (Fronts::new(calls), back)
    }

    /// Answers lookups until every [`Fronts`] is dropped.
    pub async fn run(mut self) {
        logging::info!(name: logging::event::SITES_SERVING, ttl_us = logging::Took(self.found.ttl()).field());
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
                    if reply.send(answer).is_err() {
                        logging::debug!(name: logging::event::REPLY_DROPPED, lookup = "serve");
                    }
                }
                Ask::Suspended(reply) => {
                    let answer = self.notice(&hostname).await;
                    if let Err(failure) = &answer {
                        logging::warn!(name: logging::event::SITES_FAILED, error = logging::Failure(failure).field());
                    }
                    if reply.send(answer).is_err() {
                        logging::debug!(name: logging::event::REPLY_DROPPED, lookup = "suspended");
                    }
                }
            }
        }
        logging::info!(name: logging::event::SITES_STOPPED, asked = logging::Count(asked).field());
    }

    /// What `hostname` serves now.
    async fn answer(&self, hostname: &Hostname) -> Answer {
        let Some(loaded) = self.found.resolve(hostname).await? else { return Ok(None) };
        let scope = self.place(&loaded)?;
        Ok(Some(Resolved { deploy: loaded.deploy, scope }))
    }

    /// The notice of `hostname`'s site, when the site is suspended.
    async fn notice(&self, hostname: &Hostname) -> Noticed {
        self.found.notice(hostname).await
    }

    /// The deploy's blobs placed in the store's files, once for each deploy.
    fn place(&self, loaded: &Loaded) -> Result<Arc<Placed>, Failure> {
        let key = (loaded.site, loaded.deploy.manifest.id());
        let kept = self.placed.borrow().get(&key).cloned();
        if let Some(placed) = kept {
            return Ok(placed);
        }
        let lookup = |error: &(dyn std::error::Error + 'static)| Failure::Lookup(Chain::of(error));
        let tenant: &Tenant = &loaded.tenant;
        let mut files = BTreeMap::new();
        for entry in loaded.deploy.manifest.entries() {
            let path = ContentPath::parse(entry.blob().to_string()).map_err(|error| lookup(&error))?;
            files.insert(entry.blob(), path);
        }
        let placed = Arc::new(Placed::new(self.locate.folder(tenant), files));
        self.placed.borrow_mut().put(key, Arc::clone(&placed));
        Ok(placed)
    }
}
