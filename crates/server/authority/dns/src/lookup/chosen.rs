//! The resolver a server runs with, chosen by its operator: none, the system's nameservers, or two
//! sets of nameservers that must agree.
//! One concrete type, so the API and the dashboard share it without a generic parameter each. The
//! system's nameservers are the `system` feature, so a build without sockets has `Offline` only.

#[cfg(feature = "system")]
use std::time::Duration;

#[cfg(feature = "system")]
use clock::System as Entropy;

#[cfg(feature = "system")]
use super::agreeing::Agreeing;
use super::failed::Failed;
#[cfg(feature = "system")]
use super::system::{Servers, System};
use crate::{Host, NoResolver, Resolver};

/// A resolver chosen at start.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Chosen {
    /// None: a proof says why it cannot be checked.
    Offline,
    /// Nameservers, asked within a limit.
    #[cfg(feature = "system")]
    System(System<Entropy>),
    /// Two sets of nameservers that must give the same records.
    #[cfg(feature = "system")]
    Agreeing(Agreeing<Entropy>),
}

impl Chosen {
    /// The resolver that asks `servers`, giving a lookup at most `limit`.
    #[cfg(feature = "system")]
    pub const fn system(servers: Servers, limit: Duration) -> Self {
        Self::System(System::new(servers, limit, Entropy))
    }

    /// The resolver that asks `servers` and `confirm`, each giving a lookup at most `limit`, and
    /// answers only what both say.
    #[cfg(feature = "system")]
    pub const fn agreeing(servers: Servers, confirm: Servers, limit: Duration) -> Self {
        Self::Agreeing(Agreeing::new(System::new(servers, limit, Entropy), System::new(confirm, limit, Entropy)))
    }
}

impl Resolver for Chosen {
    type Error = Failed;

    #[cfg_attr(
        not(feature = "system"),
        expect(
            unused_variables,
            clippy::unused_async_trait_impl,
            reason = "only the system resolver reads the host and waits"
        )
    )]
    async fn txt(&self, host: &Host) -> Result<Vec<String>, Failed> {
        match self {
            Self::Offline => Err(Failed::Offline(NoResolver)),
            #[cfg(feature = "system")]
            Self::System(system) => system.txt(host).await,
            #[cfg(feature = "system")]
            Self::Agreeing(agreeing) => agreeing.txt(host).await,
        }
    }
}
