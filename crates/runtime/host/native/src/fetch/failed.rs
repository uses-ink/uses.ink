use std::path::PathBuf;

use crate::error::Refused as Unread;

/// Why a fetch or a read failed with no case of its own.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failed {
    /// The cache folder could not be made, or a clone could not be removed or measured.
    Cache {
        /// The folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// git could not fetch: the remote is unreachable or sent something unusable.
    Fetch {
        /// What git said.
        source: gix::Error,
    },
    /// A clone the cache holds could not be opened.
    Clone {
        /// What git said.
        source: gix::Error,
    },
    /// The fetch ran past the wall-clock deadline of the call that asked for it.
    Late,
    /// The fetched commit's tree or history could not be read.
    Read(Unread),
}

impl std::fmt::Display for Failed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cache { path, .. } => write!(out, "the clone cache {} could not be used", path.display()),
            Self::Fetch { .. } => out.write_str("the repository could not be fetched"),
            Self::Clone { .. } => out.write_str("a clone held in the cache could not be opened"),
            Self::Late => out.write_str("the fetch ran past the time of the call that asked for it"),
            Self::Read(_) => out.write_str("the fetched commit could not be read"),
        }
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache { source, .. } => Some(source),
            Self::Fetch { source } | Self::Clone { source } => Some(source),
            Self::Read(source) => Some(source),
            Self::Late => None,
        }
    }
}
