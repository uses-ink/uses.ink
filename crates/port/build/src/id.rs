//! A build's identity.

use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One run of the pipeline: 128 bits the control plane draws once and never reuses, shown as 32
/// lower-case hex digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuildId([u8; 16]);

impl BuildId {
    /// A build id from its 16 bytes.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The 16 bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for BuildId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

impl Serialize for BuildId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BuildId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(D::Error::custom)
    }
}

/// Text that is no build id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildIdError {
    /// Not 32 characters long.
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

impl fmt::Display for BuildIdError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a build id is 32 hex digits, not {found}"),
            Self::Digit { at } => write!(out, "byte {at} of the build id is not a lower-case hex digit"),
        }
    }
}

impl std::error::Error for BuildIdError {}

impl FromStr for BuildId {
    type Err = BuildIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 32 {
            return Err(BuildIdError::Length { found: text.len() });
        }
        let digit = |at: usize| match text.as_bytes().get(at) {
            Some(&b @ b'0'..=b'9') => Ok(b - b'0'),
            Some(&b @ b'a'..=b'f') => Ok(b - b'a' + 10),
            _ => Err(BuildIdError::Digit { at }),
        };
        let mut bytes = [0_u8; 16];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = digit(2 * i)? << 4 | digit(2 * i + 1)?;
        }
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn writes_and_reads_hex() {
        let id = BuildId::from_bytes([0xab; 16]);
        assert_eq!(id.to_string(), "ab".repeat(16));
        assert_eq!(id.to_string().parse::<BuildId>().unwrap(), id);
        assert!("AB".repeat(16).parse::<BuildId>().is_err());
        assert_eq!("a".parse::<BuildId>().unwrap_err(), BuildIdError::Length { found: 1 });
    }
}
