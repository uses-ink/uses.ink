//! The host to deploy port over the control plane.

use std::future::Future;

use blob::Tenant;
use clock::Clock;
use control::{Control, Ports};
use edge::{Hostname, Notice, Resolved, Sites};
use edge_control::{Failure, Found, Settings};
use send_wrapper::SendWrapper;

/// [`Sites`] over a control plane: what a host serves is the live deploy of the site its handle,
/// preview label or proved custom domain names (the lookup of `ink serve`, [`Found`]), and a
/// suspended site has a notice. Its scope is the tenant whose blobs the deploy's are.
#[derive(Debug)]
pub struct Sited<P: Ports, K> {
    found: SendWrapper<Found<P, K>>,
}

impl<P: Ports + 'static, K: Clock> Sited<P, K> {
    /// The lookup over `control`, timed by `clock` and kept as `settings` say.
    pub fn new(control: Control<P>, clock: K, settings: &Settings) -> Self {
        Self { found: SendWrapper::new(Found::new(control, clock, settings)) }
    }
}

impl<P: Ports + 'static, K: Clock + 'static> Sites for Sited<P, K> {
    type Scope = Tenant;
    type Error = Failure;

    fn resolve(&self, hostname: &Hostname) -> impl Future<Output = Result<Option<Resolved<Tenant>>, Failure>> + Send {
        SendWrapper::new(async move {
            let loaded = self.found.resolve(hostname).await?;
            Ok(loaded.map(|loaded| Resolved { deploy: loaded.deploy, scope: loaded.tenant }))
        })
    }

    fn suspended(&self, hostname: &Hostname) -> impl Future<Output = Result<Option<Notice>, Failure>> + Send {
        SendWrapper::new(self.found.notice(hostname))
    }
}
