//! What sealing and opening fail with.

use std::fmt;

/// A seal or open that failed. `E` is the adapter's own error, kept as the source.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// The sealed value does not open: it was altered, sealed under another key, or bound to a
    /// different row. The adapter says no more, so a failure teaches an attacker nothing.
    Refused,
    /// The sealed bytes are not in any format the adapter wrote.
    Malformed,
    /// The adapter failed.
    Backend {
        /// The adapter's error.
        source: E,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused => out.write_str("the sealed value did not open for this row"),
            Self::Malformed => out.write_str("the sealed value is not in a format this adapter wrote"),
            Self::Backend { .. } => out.write_str("sealing failed"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend { source } => Some(source),
            Self::Refused | Self::Malformed => None,
        }
    }
}
