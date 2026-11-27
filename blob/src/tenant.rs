//! Whose blobs they are.

use std::fmt;
use std::str::FromStr;

/// The owner of a set of blobs: a stable id of one site's, never its handle, because a rename
/// keeps identity. Lower-case letters, digits and hyphens, 1 to 64 of them, not starting with a
/// hyphen, so it is safe as a directory name or a key segment by construction.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tenant(String);

impl Tenant {
    /// The most characters a tenant has.
    pub const MAX: usize = 64;

    /// The tenant as text: a key segment.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tenant {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl AsRef<str> for Tenant {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Text that is no tenant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TenantError {
    /// Empty.
    Empty,
    /// More than [`Tenant::MAX`] characters.
    Long {
        /// How many it has.
        found: usize,
    },
    /// A character outside lower-case letters, digits and hyphens, at this byte offset, or a
    /// hyphen first.
    Character {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for TenantError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a tenant is not empty"),
            Self::Long { found } => write!(out, "a tenant has at most {} characters, not {found}", Tenant::MAX),
            Self::Character { at } => {
                write!(out, "byte {at} of the tenant is not a lower-case letter, a digit or a hyphen after the first")
            }
        }
    }
}

impl std::error::Error for TenantError {}

impl FromStr for Tenant {
    type Err = TenantError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(TenantError::Empty);
        }
        if text.len() > Self::MAX {
            return Err(TenantError::Long { found: text.len() });
        }
        let bad = text
            .bytes()
            .enumerate()
            .find(|&(at, b)| !(b.is_ascii_lowercase() || b.is_ascii_digit() || (b == b'-' && at > 0)));
        match bad {
            Some((at, _)) => Err(TenantError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn accepts_only_what_is_safe_as_a_path_segment() {
        for ok in ["a", "0189-abc", &"a".repeat(64)] {
            assert!(ok.parse::<Tenant>().is_ok(), "{ok}");
        }
        for bad in ["", "..", ".", "a/b", "a\\b", "-a", "A", "a b", "a\0", "\u{e9}", "a.b", "~"] {
            assert!(bad.parse::<Tenant>().is_err(), "{bad:?}");
        }
        assert_eq!("a".repeat(65).parse::<Tenant>().unwrap_err(), TenantError::Long { found: 65 });
    }
}
