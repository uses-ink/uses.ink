//! What a build call fails with.

use std::fmt;

use crate::Kind;

/// A build that failed, as opposed to a site that did not build. `E` is the adapter's own error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// This builder cannot read this kind of source.
    Unsupported {
        /// The kind.
        kind: Kind,
    },
    /// The builder failed.
    Backend {
        /// The adapter's error.
        source: E,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { kind } => write!(out, "this builder cannot read a {kind} source"),
            Self::Backend { .. } => out.write_str("the builder failed"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend { source } => Some(source),
            Self::Unsupported { .. } => None,
        }
    }
}
