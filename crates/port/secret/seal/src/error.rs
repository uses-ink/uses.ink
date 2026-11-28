//! What sealing, opening and reading a key can fail with.

use std::fmt;

/// Why a credential could not be sealed.
#[derive(Debug)]
pub enum Sealing {
    /// The operating system gave no randomness for the nonce.
    Random {
        /// What it said.
        source: rand_core::Error,
    },
    /// The cipher refused. It gives no reason, so none is kept: it only fails on a message far
    /// past any credential's size.
    Cipher,
}

impl fmt::Display for Sealing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Random { .. } => out.write_str("the operating system gave no randomness"),
            Self::Cipher => out.write_str("the credential could not be sealed"),
        }
    }
}

impl std::error::Error for Sealing {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Random { source } => Some(source),
            Self::Cipher => None,
        }
    }
}

/// The operating system gave no randomness for a key.
#[derive(Debug)]
pub struct Entropy {
    /// What it said.
    pub source: rand_core::Error,
}

impl fmt::Display for Entropy {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the operating system gave no randomness")
    }
}

impl std::error::Error for Entropy {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Why a sealed value did not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    /// It was altered, sealed under another key, or bound to a different row. No more is said, so
    /// a failure teaches an attacker nothing.
    Refused,
    /// It is not in the format this code writes.
    Malformed,
}

impl fmt::Display for Opening {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused => out.write_str("the sealed value did not open for this row"),
            Self::Malformed => out.write_str("the sealed value is not in a format this code wrote"),
        }
    }
}

impl std::error::Error for Opening {}

impl<E> From<Opening> for secret::Error<E> {
    fn from(opening: Opening) -> Self {
        match opening {
            Opening::Refused => Self::Refused,
            Opening::Malformed => Self::Malformed,
        }
    }
}

/// Text that is no key: a key is 64 hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unhexed {
    /// Not 64 characters.
    Length {
        /// How many it has.
        found: usize,
    },
    /// A character that is not a hexadecimal digit, at this byte offset.
    Digit {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for Unhexed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a key is 64 hexadecimal digits, not {found} characters"),
            Self::Digit { at } => write!(out, "byte {at} of the key is not a hexadecimal digit"),
        }
    }
}

impl std::error::Error for Unhexed {}
