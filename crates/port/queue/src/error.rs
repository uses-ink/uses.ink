//! What a queue call fails with.

use std::fmt;

/// A queue call that failed. `E` is the adapter's own error, kept as the source.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// The queue failed.
    Backend {
        /// The adapter's error.
        source: E,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend { .. } => out.write_str("the queue failed"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend { source } => Some(source),
        }
    }
}
