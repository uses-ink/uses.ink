//! What a cache call cannot do, and why a job went without the cache.

use std::fmt;
use std::path::PathBuf;

/// What a cache call cannot do.
#[derive(Debug)]
pub(crate) enum Fault {
    Io { path: PathBuf, source: std::io::Error },
    Json { path: PathBuf, source: serde_json::Error },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } => write!(out, "cannot use the cache file {}", path.display()),
            Self::Json { path, .. } => write!(out, "the cache file {} is not what the farm wrote", path.display()),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
        }
    }
}

/// Why the cache was not used for a job: said once, and the job runs as if there were none.
#[derive(Debug)]
pub(crate) enum Trouble {
    Inputs(crate::cache::inputs::Fault),
    Store(Fault),
    /// Waiting for an identical job failed.
    Attach(crate::cli::error::Error),
    /// There is no cache.
    Off,
}

impl From<crate::cache::inputs::Fault> for Trouble {
    fn from(fault: crate::cache::inputs::Fault) -> Self {
        Self::Inputs(fault)
    }
}

impl From<Fault> for Trouble {
    fn from(fault: Fault) -> Self {
        Self::Store(fault)
    }
}

impl fmt::Display for Trouble {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inputs(_) => write!(out, "the inputs of the job could not be digested"),
            Self::Store(_) => write!(out, "the cache could not be used"),
            Self::Attach(_) => write!(out, "attaching to the identical job failed"),
            Self::Off => write!(out, "there is no cache directory"),
        }
    }
}

impl std::error::Error for Trouble {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inputs(source) => Some(source),
            Self::Store(source) => Some(source),
            Self::Attach(source) => Some(source),
            Self::Off => None,
        }
    }
}

/// An error and its causes on one line: `a: caused by b: caused by c`.
#[derive(Debug)]
pub(crate) struct Causes<'a>(pub(crate) &'a (dyn std::error::Error + 'static));

impl fmt::Display for Causes<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(error) = cause {
            write!(out, "; caused by: {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}
