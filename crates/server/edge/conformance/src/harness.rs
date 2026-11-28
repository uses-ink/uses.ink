//! What an adapter gives the suite.

use std::error::Error;
use std::fmt;

use edge::{Blobs, Edge, Sites};
use model::Manifest;

use crate::Seed;

/// What an adapter can do beyond serving a deploy. A case that needs more than it offers is not run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offers {
    /// A site can be suspended, and its host then answers a notice.
    pub suspensions: bool,
    /// A custom domain can be made to serve a site.
    pub domains: bool,
}

/// A seeding step that failed: the harness could not make what the case asks for. It is a fault of
/// the harness or its store, and it stops the case.
#[derive(Debug)]
pub struct Unseeded {
    /// Which step.
    pub step: &'static str,
    /// What the store said.
    pub source: Box<dyn Error>,
}

/// A host the harness never seeded.
#[derive(Debug)]
struct Unknown(String);

impl fmt::Display for Unknown {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "no site was seeded at {}", self.0)
    }
}

impl Error for Unknown {}

impl Unseeded {
    /// The failure of a step that names a host nothing was seeded at.
    pub fn unknown(host: &str) -> Self {
        Self { step: "find the site of a host", source: Box::new(Unknown(host.to_owned())) }
    }

    /// The conversion of a store's error into the failure of `step`.
    pub fn because<E: Error + 'static>(step: &'static str) -> impl FnOnce(E) -> Self {
        move |source| Self { step, source: Box::new(source) }
    }
}

impl fmt::Display for Unseeded {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the harness could not {}", self.step)
    }
}

impl Error for Unseeded {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// An adapter under test: its two ports, the edge over them, and the means to seed them.
#[expect(async_fn_in_trait, reason = "a Worker's futures are not Send, and a runner awaits them where it is")]
pub trait Harness {
    /// What the blobs are read under.
    type Scope: Send + Sync;
    /// The host to deploy port.
    type Sites: Sites<Scope = Self::Scope>;
    /// The blob port.
    type Blobs: Blobs<Scope = Self::Scope>;

    /// The adapter of the first port.
    fn sites(&self) -> &Self::Sites;

    /// The adapter of the second.
    fn blobs(&self) -> &Self::Blobs;

    /// An edge over the two, with its default limits.
    fn edge(&self) -> Result<Edge<Self::Sites, Self::Blobs>, edge::Error>;

    /// What the adapter offers beyond serving.
    fn offers(&self) -> Offers;

    /// Makes `seed.host` serve the pages, as the live deploy of a site of its own, and returns the manifest.
    async fn publish(&self, seed: &Seed) -> Result<Manifest, Unseeded>;

    /// Suspends the site `host` serves. Only called when [`Offers::suspensions`].
    async fn suspend(&self, host: &str) -> Result<(), Unseeded>;

    /// Makes the name `custom` serve the site `host` serves. Only called when [`Offers::domains`].
    async fn alias(&self, host: &str, custom: &str) -> Result<(), Unseeded>;
}
