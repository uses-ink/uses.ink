//! The port.

use std::path::PathBuf;

use build::Revision;
use https::Https;
use model::git::{Locator, Network, Scheme};
use secret::Plain;
use watch::{Access, Head, Heads};

use crate::Unlisted;
use crate::listing::{Listing, Login};

/// Remotes read through git's own protocol (`file` and `https`), within the operator's network setting.
#[derive(Clone, Debug)]
pub struct Remotes {
    scratch: PathBuf,
    network: Network,
    https: Https,
}

impl Remotes {
    /// Remotes listed from a repository kept in `scratch`, which only schemes `network` allows
    /// may be reached.
    pub fn new(scratch: impl Into<PathBuf>, network: Network) -> Self {
        Self { scratch: scratch.into(), network, https: Https::default() }
    }

    /// The same remotes with these https settings: timeouts, redirects and extra trusted roots.
    #[must_use]
    pub fn with_https(self, https: Https) -> Self {
        Self { https, ..self }
    }
}

impl logging::Kind for &Unlisted {
    fn kind(self) -> &'static str {
        match self {
            Unlisted::Network { .. } => "network",
            Unlisted::Https(_) => "https",
            Unlisted::Scratch { .. } => "scratch",
            Unlisted::Remote { .. } => "remote",
            Unlisted::Missing(_) => "missing",
            Unlisted::Commit(_) => "commit",
            Unlisted::Task(_) => "task",
        }
    }
}

impl Remotes {
    /// Says how a read went. A read is one poll of one site, which the watch reports itself
    /// (`watch.polled`), so a failure here is `debug` detail; only a refusal by the operator's
    /// network setting is a `warn`.
    fn said(scheme: Scheme, revision: Option<&Revision>, read: &Result<Head, Unlisted>) {
        let (scheme, named) = (logging::Shown(scheme), revision.is_some());
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

    /// The head, listed from the remote or refused before any connection.
    async fn read(
        &self,
        url: &Locator,
        revision: Option<&Revision>,
        access: Option<&Access>,
    ) -> Result<Head, Unlisted> {
        let scheme = url.scheme();
        if !self.network.allows(scheme) {
            return Err(Unlisted::Network { scheme, policy: self.network });
        }
        let login =
            access.map(|access| Login { user: access.user().to_owned(), password: Plain::from(access.password()) });
        let listing = Listing {
            scratch: self.scratch.clone(),
            url: url.to_string(),
            revision: revision.map(ToString::to_string),
            login,
            https: (scheme == Scheme::Https).then(|| self.https.clone()),
        };
        tokio::task::spawn_blocking(move || listing.run()).await.map_err(Unlisted::Task)?
    }
}

impl Heads for Remotes {
    type Error = Unlisted;

    async fn head(
        &self,
        url: &Locator,
        revision: Option<&Revision>,
        access: Option<&Access>,
    ) -> Result<Head, Unlisted> {
        let read = self.read(url, revision, access).await;
        Self::said(url.scheme(), revision, &read);
        read
    }
}
