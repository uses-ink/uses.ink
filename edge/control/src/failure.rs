//! Why a lookup through the channel failed.

use std::error::Error;
use std::fmt;

use crate::Chain;

/// A lookup that could not be answered.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failure {
    /// The half that holds the control plane is gone: its task ended.
    Gone,
    /// The control plane failed, or a deploy it holds could not be read.
    Lookup(Chain),
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gone => out.write_str("the control plane's task has ended, so no host can be looked up"),
            Self::Lookup(_) => out.write_str("the control plane could not say what the host serves"),
        }
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Gone => None,
            Self::Lookup(chain) => Some(chain),
        }
    }
}
