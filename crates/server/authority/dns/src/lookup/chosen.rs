//! The resolver a server runs with, chosen by its operator: none, or the system's nameservers.
//! One concrete type, so the API and the dashboard share it without a generic parameter each.

use std::time::Duration;

use clock::System as Entropy;

use super::failed::Failed;
use super::system::{Servers, System};
use crate::{Host, NoResolver, Resolver};

/// A resolver chosen at start.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Chosen {
    /// None: a proof says why it cannot be checked.
    Offline,
    /// Nameservers, asked within a limit.
    System(System<Entropy>),
}

impl Chosen {
    /// The resolver that asks `servers`, giving a lookup at most `limit`.
    pub const fn system(servers: Servers, limit: Duration) -> Self {
        Self::System(System::new(servers, limit, Entropy))
    }
}

impl Resolver for Chosen {
    type Error = Failed;

    async fn txt(&self, host: &Host) -> Result<Vec<String>, Failed> {
        match self {
            Self::Offline => Err(Failed::Offline(NoResolver)),
            Self::System(system) => system.txt(host).await,
        }
    }
}
