//! Typed ids: 128 bits drawn by the caller, shown as 32 lower-case hex digits.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::{Serialize, Serializer};

/// What an [`Id`] identifies. A marker, never constructed.
pub trait Kind {
    /// The noun, for saying which id was wrong.
    const NOUN: &'static str;
}

/// An account.
#[derive(Clone, Copy, Debug)]
pub enum Account {}
/// A site.
#[derive(Clone, Copy, Debug)]
pub enum Site {}
/// A stored credential.
#[derive(Clone, Copy, Debug)]
pub enum Credential {}

impl Kind for Account {
    const NOUN: &'static str = "account";
}
impl Kind for Site {
    const NOUN: &'static str = "site";
}
impl Kind for Credential {
    const NOUN: &'static str = "credential";
}

/// The identity of one `K`. The control plane never draws one: the caller does, from whatever
/// randomness it has (a host capability), so the same calls make the same rows in a test.
pub struct Id<K>([u8; 16], PhantomData<fn() -> K>);

impl<K> Id<K> {
    /// An id from its 16 bytes.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes, PhantomData)
    }

    /// The 16 bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl<K> Clone for Id<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Id<K> {}

impl<K> PartialEq for Id<K> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K> Eq for Id<K> {}

impl<K> Hash for Id<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<K> PartialOrd for Id<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> Ord for Id<K> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<K> fmt::Display for Id<K> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

impl<K: Kind> fmt::Debug for Id<K> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}:{self}", K::NOUN)
    }
}

impl<K> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Text that is no id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdError {
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

impl fmt::Display for IdError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "an id is 32 hex digits, not {found}"),
            Self::Digit { at } => write!(out, "byte {at} of the id is not a lower-case hex digit"),
        }
    }
}

impl std::error::Error for IdError {}

impl<K> FromStr for Id<K> {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 32 {
            return Err(IdError::Length { found: text.len() });
        }
        let digit = |at: usize| match text.as_bytes().get(at) {
            Some(&b @ b'0'..=b'9') => Ok(b - b'0'),
            Some(&b @ b'a'..=b'f') => Ok(b - b'a' + 10),
            _ => Err(IdError::Digit { at }),
        };
        let mut bytes = [0_u8; 16];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = digit(2 * i)? << 4 | digit(2 * i + 1)?;
        }
        Ok(Self::from_bytes(bytes))
    }
}

impl<K> TryFrom<Id<K>> for blob::Tenant {
    type Error = blob::TenantError;

    /// A site's blobs are keyed by its id, never its handle, so a rename keeps them.
    fn try_from(id: Id<K>) -> Result<Self, Self::Error> {
        id.to_string().parse()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn round_trips_as_hex() {
        let id = Id::<Site>::from_bytes([0x0f; 16]);
        assert_eq!(id.to_string(), "0f".repeat(16));
        assert_eq!(id.to_string().parse::<Id<Site>>().unwrap(), id);
        assert_eq!(format!("{id:?}"), format!("site:{id}"));
        assert!("xx".repeat(16).parse::<Id<Site>>().is_err());
    }

    #[test]
    fn is_a_tenant() {
        let id = Id::<Site>::from_bytes([0xab; 16]);
        assert_eq!(blob::Tenant::try_from(id).unwrap().as_str(), id.to_string());
    }
}
