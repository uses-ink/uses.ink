//! The port.

use build::Revision;
use logging::Field as _;
use model::git::{Locator, Network, Scheme};
use watch::{Access, Branches, Head, Heads};

use crate::advert::HeadOr;
use crate::{Advertisement, Basic, Fetch, Get, Unlisted};

/// Reads heads from the smart-HTTP reference list of a repository, over `F`.
#[derive(Clone, Debug)]
pub struct Smart<F> {
    fetch: F,
    network: Network,
    most: usize,
}

impl<F: Fetch> Smart<F> {
    /// The most bytes of reference list read when none is set: 8 MiB. A repository with tens of
    /// thousands of references is well under it.
    pub const MOST: usize = 8 << 20;

    /// Heads read through `fetch`, within the operator's `network` setting.
    pub const fn new(fetch: F, network: Network) -> Self {
        Self { fetch, network, most: Self::MOST }
    }

    /// The same, refusing a reference list over `most` bytes.
    #[must_use]
    pub fn most_of(self, most: usize) -> Self {
        Self { most, ..self }
    }

    /// The fetch, for a test that reads what it saw.
    pub const fn fetch(&self) -> &F {
        &self.fetch
    }

    /// The advertisement of `url`, or why there is none.
    async fn advertised(&self, url: &Locator, access: Option<&Access>) -> Result<Advertisement, Unlisted<F::Error>> {
        let scheme = url.scheme();
        if !self.network.allows(scheme) {
            return Err(Unlisted::Network { scheme, policy: self.network });
        }
        if scheme != Scheme::Https {
            return Err(Unlisted::Scheme { scheme });
        }
        let base = url.to_string();
        let get = Get {
            url: format!("{}/info/refs?service=git-upload-pack", base.trim_end_matches('/')),
            basic: access.map(|access| Basic::new(access.user(), access.password())),
        };
        let reply = self.fetch.get(&get).await.map_err(Unlisted::Fetch)?;
        if reply.status != 200 {
            return Err(Unlisted::Status { code: reply.status });
        }
        if reply.body.len() > self.most {
            return Err(Unlisted::Large { most: self.most });
        }
        Advertisement::parse(&reply.body).map_err(Unlisted::Advert)
    }

    /// Says how a read went: a poll is reported by the watch itself, so a failure is `debug`
    /// detail, and only a refusal by the network setting is a `warn` (as `watch/git` does).
    fn said(scheme: Scheme, named: bool, read: &Result<Head, Unlisted<F::Error>>) {
        let scheme = logging::Shown(scheme);
        match read {
            Ok(head) => logging::debug!(
                name: logging::event::GIT_HEAD,
                scheme = scheme.field(),
                named,
                outcome = "read",
                head = logging::Shown(head).field(),
            ),
            Err(error @ Unlisted::Network { .. }) => logging::warn!(
                name: logging::event::GIT_HEAD,
                scheme = scheme.field(),
                named,
                outcome = logging::Kind::kind(error),
                error = logging::Failure(error).field(),
            ),
            Err(error) => logging::debug!(
                name: logging::event::GIT_HEAD,
                scheme = scheme.field(),
                named,
                outcome = logging::Kind::kind(error),
                error = logging::Failure(error).field(),
            ),
        }
    }
}

impl<E> logging::Kind for &Unlisted<E> {
    fn kind(self) -> &'static str {
        match self {
            Unlisted::Network { .. } => "network",
            Unlisted::Scheme { .. } => "scheme",
            Unlisted::Fetch(_) => "remote",
            Unlisted::Status { .. } => "status",
            Unlisted::Large { .. } => "large",
            Unlisted::Advert(_) => "advert",
            Unlisted::Absent(_) => "missing",
            Unlisted::Commit(_) => "commit",
        }
    }
}

impl<F: Fetch> Heads for Smart<F> {
    type Error = Unlisted<F::Error>;

    async fn head(
        &self,
        url: &Locator,
        revision: Option<&Revision>,
        access: Option<&Access>,
    ) -> Result<Head, Self::Error> {
        let read = match self.advertised(url, access).await {
            Ok(advert) => advert.head(revision.map(ToString::to_string).as_deref()).map_err(|error| match error {
                HeadOr::Absent(absent) => Unlisted::Absent(absent),
                HeadOr::Commit(source) => Unlisted::Commit(source),
            }),
            Err(error) => Err(error),
        };
        Self::said(url.scheme(), revision.is_some(), &read);
        read
    }

    async fn branches(&self, url: &Locator, access: Option<&Access>) -> Result<Branches, Self::Error> {
        self.advertised(url, access).await?.branches().map_err(Unlisted::Commit)
    }
}
