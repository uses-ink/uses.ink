//! The name that owns `<handle>.uses.ink`.

use std::fmt;
use std::str::FromStr;

use db::Row;
use sea_query::IdenStatic;
use serde::Serialize;

use crate::fields::Fields;
use crate::{BrandError, Brands, Unreadable};

/// A handle: a lower-case DNS label of 3 to 32 characters, `[a-z0-9-]`, with no leading or
/// trailing hyphen and no `--` (architecture section 12). Reserved names are valid handles; the
/// control plane refuses to hand them out ([`Handle::is_reserved`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Handle(String);

impl Handle {
    /// The handle in `column` of `row`.
    pub(crate) fn stored(row: &Row, column: impl IdenStatic) -> Result<Self, Unreadable> {
        Fields(row).text(column)?.parse().map_err(|source| Unreadable::Handle { source })
    }

    /// The fewest characters a handle has.
    pub const MIN: usize = 3;
    /// The most characters a handle has.
    pub const MAX: usize = 32;

    /// Names kept for uses.ink itself: infrastructure and route nouns (decision 0045 lists
    /// them; the brand list is the owner's, question O3, and is not here yet).
    pub const RESERVED: [&'static str; 24] = [
        "www",
        "app",
        "api",
        "docs",
        "status",
        "mail",
        "admin",
        "static",
        "cdn",
        "assets",
        "domains",
        "edge",
        "blog",
        "help",
        "support",
        "login",
        "logout",
        "account",
        "dashboard",
        "ink",
        "uses",
        "root",
        "abuse",
        "security",
    ];

    /// The handle as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the name is one of the infrastructure names of [`Handle::RESERVED`].
    pub fn is_reserved(&self) -> bool {
        Self::RESERVED.contains(&self.0.as_str())
    }

    /// Why the name is kept, if it is: an infrastructure name, or a name of the brand list.
    pub fn reservation(&self) -> Result<Option<Reservation>, BrandError> {
        if self.is_reserved() {
            return Ok(Some(Reservation::Infrastructure));
        }
        Ok(Brands::bundled()?.holds(self).then_some(Reservation::Brand))
    }
}

/// Why a name cannot be claimed whoever asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reservation {
    /// The name is infrastructure or a route noun of uses.ink itself.
    Infrastructure,
    /// The name is on the brand list.
    Brand,
}

impl AsRef<str> for Handle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Text that is no handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HandleError {
    /// Fewer than [`Handle::MIN`] or more than [`Handle::MAX`] characters.
    Length {
        /// How many it has.
        found: usize,
    },
    /// A character outside lower-case letters, digits and hyphens, at this byte offset.
    Character {
        /// The byte offset.
        at: usize,
    },
    /// A hyphen first or last.
    Edge,
    /// Two hyphens in a row, which are kept for previews.
    Double {
        /// The byte offset of the first.
        at: usize,
    },
}

impl fmt::Display for HandleError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => {
                write!(out, "a handle has {} to {} characters, not {found}", Handle::MIN, Handle::MAX)
            }
            Self::Character { at } => {
                write!(out, "byte {at} of the handle is not a lower-case letter, a digit or a hyphen")
            }
            Self::Edge => out.write_str("a handle does not start or end with a hyphen"),
            Self::Double { at } => write!(out, "a handle has no two hyphens in a row (at byte {at})"),
        }
    }
}

impl std::error::Error for HandleError {}

impl FromStr for Handle {
    type Err = HandleError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !(Self::MIN..=Self::MAX).contains(&text.len()) {
            return Err(HandleError::Length { found: text.len() });
        }
        if let Some(at) = text.bytes().position(|b| !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')) {
            return Err(HandleError::Character { at });
        }
        if text.starts_with('-') || text.ends_with('-') {
            return Err(HandleError::Edge);
        }
        if let Some(at) = text.find("--") {
            return Err(HandleError::Double { at });
        }
        Ok(Self(text.to_owned()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn follows_the_dns_label_rules() {
        for ok in ["abc", "a-b", "cstef", "a1b2", &"a".repeat(32)] {
            assert!(ok.parse::<Handle>().is_ok(), "{ok}");
        }
        assert_eq!("ab".parse::<Handle>().unwrap_err(), HandleError::Length { found: 2 });
        assert_eq!("a".repeat(33).parse::<Handle>().unwrap_err(), HandleError::Length { found: 33 });
        assert_eq!("Abc".parse::<Handle>().unwrap_err(), HandleError::Character { at: 0 });
        assert_eq!("a_c".parse::<Handle>().unwrap_err(), HandleError::Character { at: 1 });
        assert_eq!("-ab".parse::<Handle>().unwrap_err(), HandleError::Edge);
        assert_eq!("ab-".parse::<Handle>().unwrap_err(), HandleError::Edge);
        assert_eq!("a--b".parse::<Handle>().unwrap_err(), HandleError::Double { at: 1 });
        assert!("xn--a".parse::<Handle>().is_err());
    }

    #[test]
    fn reserves_infrastructure_names() {
        assert!("www".parse::<Handle>().unwrap().is_reserved());
        assert!(!"cstef".parse::<Handle>().unwrap().is_reserved());
    }
}
