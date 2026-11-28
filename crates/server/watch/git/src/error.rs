//! Why a remote's head could not be read.

use std::fmt;
use std::path::PathBuf;

use model::git::{Network, Scheme};

use crate::Missing;

/// A remote that gave no head.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unlisted {
    /// The operator's network setting does not allow the repository's scheme.
    Network {
        /// The scheme.
        scheme: Scheme,
        /// The setting.
        policy: Network,
    },
    /// The https settings could not be made into a connection configuration.
    Https(https::Misconfigured),
    /// The repository kept for connections could not be made or opened.
    Scratch {
        /// Where.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The remote could not be reached or did not answer as git does.
    Remote {
        /// What git said.
        source: gix::Error,
    },
    /// The remote has no commit for the revision.
    Missing(Missing),
    /// The commit the remote named is no commit id.
    Commit(watch::HeadError),
    /// The task that listed the references ended abnormally.
    Task(tokio::task::JoinError),
}

impl Unlisted {
    /// A failure of git's own, kept with its source.
    pub(crate) fn remote(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Remote { source: gix::Error::from_error(source) }
    }
}

impl fmt::Display for Unlisted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { scheme, policy } => write!(out, "the network setting `{policy}` does not allow {scheme}"),
            Self::Https(_) => out.write_str("the https connection could not be configured"),
            Self::Scratch { path, .. } => write!(out, "the repository {} could not be opened", path.display()),
            Self::Remote { .. } => out.write_str("the remote's references could not be listed"),
            Self::Missing(_) => out.write_str("the remote has no such commit"),
            Self::Commit(_) => out.write_str("the remote named a commit that is no commit id"),
            Self::Task(_) => out.write_str("the listing ended abnormally"),
        }
    }
}

impl std::error::Error for Unlisted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Scratch { source, .. } => Some(source),
            Self::Remote { source } => Some(source),
            Self::Https(source) => Some(source),
            Self::Missing(source) => Some(source),
            Self::Commit(source) => Some(source),
            Self::Task(source) => Some(source),
            Self::Network { .. } => None,
        }
    }
}
