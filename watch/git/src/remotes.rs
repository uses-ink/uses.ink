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

impl Heads for Remotes {
    type Error = Unlisted;

    async fn head(
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
