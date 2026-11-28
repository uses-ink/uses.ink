//! The half the server calls.

use std::sync::Arc;

use edge::{Hostname, Resolved, Sites};
use edge_folder::Placed;
use tokio::sync::{mpsc, oneshot};

use crate::Failure;
use crate::lookup::found::Noticed;

/// What a lookup answers.
pub(crate) type Answer = Result<Option<Resolved<Arc<Placed>>>, Failure>;

/// What a lookup asks, and where its answer goes.
#[derive(Debug)]
pub(crate) enum Ask {
    /// The deploy the host serves.
    Serve(oneshot::Sender<Answer>),
    /// The suspension of the host's site.
    Suspended(oneshot::Sender<Noticed>),
}

/// A lookup on its way to the task that holds the control plane.
#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) hostname: Hostname,
    pub(crate) ask: Ask,
}

/// The `Send` half: [`Sites`] over the channel to a [`crate::Back`]. Clones share the channel.
#[derive(Clone, Debug)]
pub struct Fronts {
    calls: mpsc::Sender<Call>,
}

impl Fronts {
    pub(crate) const fn new(calls: mpsc::Sender<Call>) -> Self {
        Self { calls }
    }
}

impl Sites for Fronts {
    type Scope = Arc<Placed>;
    type Error = Failure;

    async fn resolve(&self, hostname: &Hostname) -> Answer {
        let (reply, answered) = oneshot::channel();
        let call = Call { hostname: hostname.clone(), ask: Ask::Serve(reply) };
        self.calls.send(call).await.map_err(|_| Failure::Gone)?;
        answered.await.map_err(|_| Failure::Gone)?
    }

    async fn suspended(&self, hostname: &Hostname) -> Noticed {
        let (reply, answered) = oneshot::channel();
        let call = Call { hostname: hostname.clone(), ask: Ask::Suspended(reply) };
        self.calls.send(call).await.map_err(|_| Failure::Gone)?;
        answered.await.map_err(|_| Failure::Gone)?
    }
}
