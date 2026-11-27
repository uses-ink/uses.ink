//! Content hashes: BLAKE3, 32 bytes, shown as lower-case hex.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The BLAKE3 hash of some bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Hash([u8; 32]);

impl Hash {
    /// The number of hex digits in a written hash.
    pub const DIGITS: usize = 64;

    /// The hash of `bytes`.
    pub fn of(bytes: impl AsRef<[u8]>) -> Self {
        Self(*blake3::hash(bytes.as_ref()).as_bytes())
    }

    /// A hash already computed elsewhere.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The 32 bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<blake3::Hash> for Hash {
    fn from(hash: blake3::Hash) -> Self {
        Self(*hash.as_bytes())
    }
}

impl fmt::Display for Hash {
    /// 64 lower-case hex digits.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

/// Text that is not a written hash.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashError {
    /// Not 64 characters long.
    Length {
        /// How long it is.
        found: usize,
    },
    /// A character that is not a lower-case hex digit, at this byte offset.
    Digit {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for HashError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a hash is {} hex digits, not {found}", Hash::DIGITS),
            Self::Digit { at } => write!(out, "byte {at} of the hash is not a lower-case hex digit"),
        }
    }
}

impl std::error::Error for HashError {}

impl FromStr for Hash {
    type Err = HashError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != Self::DIGITS {
            return Err(HashError::Length { found: text.len() });
        }
        let digit = |at: usize| -> Result<u8, HashError> {
            match text.as_bytes().get(at) {
                Some(&b @ b'0'..=b'9') => Ok(b - b'0'),
                Some(&b @ b'a'..=b'f') => Ok(b - b'a' + 10),
                _ => Err(HashError::Digit { at }),
            }
        };
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = digit(2 * i)? << 4 | digit(2 * i + 1)?;
        }
        Ok(Self(bytes))
    }
}

impl TryFrom<String> for Hash {
    type Error = HashError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Hash> for String {
    fn from(hash: Hash) -> Self {
        hash.to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn round_trips_through_hex() {
        let hash = Hash::of(b"abc");
        assert_eq!(hash.to_string(), "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85");
        assert_eq!(hash.to_string().parse::<Hash>(), Ok(hash));
    }

    #[test]
    fn refuses_what_is_not_hex() {
        assert_eq!("ab".parse::<Hash>(), Err(HashError::Length { found: 2 }));
        let upper = Hash::of(b"abc").to_string().to_uppercase();
        assert_eq!(upper.parse::<Hash>(), Err(HashError::Digit { at: 4 }));
    }
}
