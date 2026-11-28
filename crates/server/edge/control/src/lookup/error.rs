//! Why a lookup through the channel failed, and the error chain it keeps so it can cross one.

use std::error::Error;
use std::fmt;

/// An error that cannot cross a thread as it is (its type holds a port's error) kept as what
/// each error in its chain said, outermost first. It is an error itself, with the same chain of
/// sources, so a caller that walks `source()` sees what the original said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
    said: String,
    cause: Option<Box<Chain>>,
}

impl Chain {
    /// `error` and every cause below it.
    pub fn of(error: &(dyn Error + 'static)) -> Self {
        Self { said: error.to_string(), cause: error.source().map(|cause| Box::new(Self::of(cause))) }
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.said)
    }
}

impl Error for Chain {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref().map(|cause| cause as &(dyn Error + 'static))
    }
}

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
