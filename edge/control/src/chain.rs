//! An error and its causes, kept so they can cross a channel.

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
