//! Which version of a source a snapshot is.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Hash;

/// The version a snapshot was taken at.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Version {
    /// A git commit.
    GitCommit(Oid),
    /// An upload, such as `ink push`'s.
    Upload(UploadId),
    /// A local folder, versioned by the hash of its tree.
    Folder {
        /// The hash of the folder's tree.
        hash: Hash,
    },
}

/// A git object id: 40 (SHA-1) or 64 (SHA-256) lower-case hex digits.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Oid(String);

/// An upload's id: 1 to 64 ASCII letters, digits or `-`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct UploadId(String);

/// Why text is not an [`Oid`] or an [`UploadId`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum VersionError {
    /// An object id of the wrong length.
    OidLength {
        /// How long it is.
        found: usize,
    },
    /// An upload id of the wrong length.
    UploadLength {
        /// How long it is.
        found: usize,
    },
    /// A character the id may not hold, at this byte offset.
    Character {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for VersionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OidLength { found } => write!(out, "a git object id is 40 or 64 hex digits, not {found}"),
            Self::UploadLength { found } => write!(out, "an upload id is 1 to 64 characters, not {found}"),
            Self::Character { at } => write!(out, "byte {at} of the id is not allowed"),
        }
    }
}

impl std::error::Error for VersionError {}

impl Oid {
    /// `text`, when it is 40 or 64 lower-case hex digits.
    pub fn parse(text: impl Into<String>) -> Result<Self, VersionError> {
        let text = text.into();
        if !matches!(text.len(), 40 | 64) {
            return Err(VersionError::OidLength { found: text.len() });
        }
        match text.bytes().position(|b| !matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Some(at) => Err(VersionError::Character { at }),
            None => Ok(Self(text)),
        }
    }

    /// The id, as git writes it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl UploadId {
    /// `text`, when it is 1 to 64 ASCII letters, digits or `-`.
    pub fn parse(text: impl Into<String>) -> Result<Self, VersionError> {
        let text = text.into();
        if !(1..=64).contains(&text.len()) {
            return Err(VersionError::UploadLength { found: text.len() });
        }
        match text.bytes().position(|b| !(b.is_ascii_alphanumeric() || b == b'-')) {
            Some(at) => Err(VersionError::Character { at }),
            None => Ok(Self(text)),
        }
    }

    /// The id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! text_newtype {
    ($name:ident) => {
        impl fmt::Display for $name {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                out.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = VersionError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Self::parse(text)
            }
        }

        impl TryFrom<String> for $name {
            type Error = VersionError;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                Self::parse(text)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

text_newtype!(Oid);
text_newtype!(UploadId);
