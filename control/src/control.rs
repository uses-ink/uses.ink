//! The control plane.

use crate::{Origin, Ports};

/// The control plane over its ports: the use cases of accounts, sites, deploys, build jobs and
/// credentials, each written once against the port traits. Nothing here names SQLite, a
/// directory, Cloudflare or a clock; the caller draws ids and says what time it is.
///
/// The use cases live beside the values they work on: `site`, `deploy`, `credential`, `job`,
/// and `remove` and `export` for the two that touch every port.
#[derive(Debug)]
pub struct Control<P> {
    pub(crate) ports: P,
    pub(crate) origin: Origin,
}

impl<P: Ports> Control<P> {
    /// A control plane on `ports`, serving sites under `origin`. The database must have had the [`Domain`](crate::Domain) migration applied.
    pub const fn new(ports: P, origin: Origin) -> Self {
        Self { ports, origin }
    }

    /// The ports.
    pub const fn ports(&self) -> &P {
        &self.ports
    }
}
