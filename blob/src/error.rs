//! What a blob call fails with.

use std::fmt;

use model::Hash;

/// A blob call that failed. `E` is the adapter's own error, kept as the source.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// A write's bytes do not hash to the hash it named. Nothing was stored.
    Mismatch {
        /// The hash the caller named.
        expected: Hash,
        /// The hash of the bytes it sent.
        found: Hash,
    },
    /// A stored blob no longer hashes to its key: the store was changed behind our back.
    Corrupt {
        /// The key.
        expected: Hash,
        /// The hash of what is stored.
        found: Hash,
    },
    /// The store failed.
    Backend {
        /// The adapter's error.
        source: E,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch { expected, found } => {
                write!(out, "the bytes hash to {found}, not to the {expected} that was named")
            }
            Self::Corrupt { expected, found } => write!(out, "the blob {expected} holds bytes that hash to {found}"),
            Self::Backend { .. } => out.write_str("the blob store failed"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend { source } => Some(source),
            Self::Mismatch { .. } | Self::Corrupt { .. } => None,
        }
    }
}
