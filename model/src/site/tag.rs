//! The tags a page carries.

use crate::Slug;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A tag as the author wrote it, with its slug for routes and comparison.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "TagParts")]
#[non_exhaustive]
pub struct Tag {
    /// What routes and comparison use: `rust-notes`.
    pub slug: Slug,
    /// As the author wrote it: `Rust Notes`.
    pub written: String,
}

/// A tag as stored, before its slug is checked against its text.
#[derive(Debug, Deserialize)]
struct TagParts {
    slug: Slug,
    written: String,
}

/// Why stored parts are no [`Tag`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TagError {
    /// The text has no letter or digit.
    Empty {
        /// Why it makes no slug.
        source: crate::SlugError,
    },
    /// The stored slug is not the slug of the text.
    Slug {
        /// The slug of the text.
        expected: Slug,
        /// The slug stored with it.
        found: Slug,
    },
}

impl fmt::Display for TagError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { .. } => out.write_str("a tag needs at least one letter or digit"),
            Self::Slug { expected, found } => {
                write!(out, "the tag's slug is `{found}`, but its text gives `{expected}`")
            }
        }
    }
}

impl std::error::Error for TagError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Empty { source } => Some(source),
            Self::Slug { .. } => None,
        }
    }
}

impl TryFrom<TagParts> for Tag {
    type Error = TagError;

    fn try_from(parts: TagParts) -> Result<Self, Self::Error> {
        let tag = Self::new(parts.written).map_err(|source| TagError::Empty { source })?;
        if tag.slug == parts.slug { Ok(tag) } else { Err(TagError::Slug { expected: tag.slug, found: parts.slug }) }
    }
}

impl Tag {
    /// A tag from what the author wrote; it needs at least one letter or digit.
    pub fn new(written: impl Into<String>) -> Result<Self, crate::SlugError> {
        let written = written.into();
        Ok(Self { slug: Slug::from_text(&written)?, written })
    }
}
