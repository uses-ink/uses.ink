//! The port itself.

use blob::Tenant;
use model::{Bytes, ExternalUrl, Manifest};

use crate::{BuildId, Error, Events, Pin, Source, Trigger};

/// What to build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The build's identity.
    pub build: BuildId,
    /// Whose blobs the deploy is stored under.
    pub tenant: Tenant,
    /// What set the build off.
    pub trigger: Trigger,
    /// Where the content comes from. For a build of a branch this already reads the branch:
    /// an adapter builds `source` and need not look at `pin`.
    pub source: Source,
    /// The branch and commit of a preview build, when it is one: what the build is of, for the
    /// adapters that name it (a container's job label, a log line).
    pub pin: Option<Pin>,
    /// Where the site will be served, for the absolute links of its feeds and sitemap.
    pub base: ExternalUrl,
}

/// A deploy made: its manifest, and every blob it names stored under the request's tenant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The manifest, stored as a blob too: its hash is the deploy's id.
    pub manifest: Manifest,
    /// The bytes the deploy serves.
    pub bytes: Bytes,
}

/// How a build ended, when the builder itself worked.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// A deploy was made.
    Built(Built),
    /// The site does not build: the events say why, and nothing was stored for a deploy.
    Refused,
}

/// Runs builds. Futures are not required to be `Send`: a Durable Object's are not.
#[expect(
    async_fn_in_trait,
    reason = "a Durable Object's futures are not Send on Workers, so no Send bound can be promised"
)]
pub trait Build {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;

    /// Builds `request`'s source, saying what happens to `events`: [`crate::Event::Started`]
    /// first and [`crate::Event::Finished`] last, always, whatever the outcome. The same source
    /// gives the same manifest, so the same deploy id.
    async fn run(&self, request: &Request, events: &mut impl Events) -> Result<Outcome, Error<Self::Error>>;
}
