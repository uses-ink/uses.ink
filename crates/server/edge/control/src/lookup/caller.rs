//! The half the server calls.

use std::sync::Arc;

use edge::{Hostname, Resolved, Sites};
use edge_folder::Placed;
use tokio::sync::{mpsc, oneshot};

use crate::Failure;

/// What a lookup answers.
pub(crate) type Answer = Result<Option<Resolved<Arc<Placed>>>, Failure>;

/// A lookup on its way to the task that holds the control plane.
#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) hostname: Hostname,
    pub(crate) reply: oneshot::Sender<Answer>,
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
        self.calls.send(Call { hostname: hostname.clone(), reply }).await.map_err(|_| Failure::Gone)?;
        answered.await.map_err(|_| Failure::Gone)?
    }
}
