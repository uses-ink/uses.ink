//! Heading levels and the anchors that link to them.

use crate::Slug;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A heading level, 1 to 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub struct Level(u8);

/// A number that is no heading level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelError {
    /// The number given.
    pub found: u8,
}

impl fmt::Display for LevelError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "a heading level is {} to {}, not {}", Level::MIN, Level::MAX, self.found)
    }
}

impl std::error::Error for LevelError {}

impl Level {
    /// The lowest heading level, `<h1>`.
    const MIN: u8 = 1;
    /// The highest heading level, `<h6>`.
    const MAX: u8 = 6;

    /// `level`, when it is 1 to 6.
    pub const fn new(level: u8) -> Result<Self, LevelError> {
        if Self::MIN <= level && level <= Self::MAX { Ok(Self(level)) } else { Err(LevelError { found: level }) }
    }

    /// The level, 1 to 6.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Level {
    type Error = LevelError;

    fn try_from(level: u8) -> Result<Self, Self::Error> {
        Self::new(level)
    }
}

impl From<Level> for u8 {
    fn from(level: Level) -> Self {
        level.0
    }
}

/// A place in a page a link can point at: a heading's slug or a block id, without the `#`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Anchor(Slug);

impl Anchor {
    /// The anchor `slug`.
    pub fn new(slug: Slug) -> Self {
        Self(slug)
    }

    /// The anchor of a heading's text.
    pub fn of_heading(text: impl AsRef<str>) -> Result<Self, crate::SlugError> {
        Slug::from_text(text).map(Self)
    }

    /// The anchor's text, without the `#`.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Slug> for Anchor {
    fn from(slug: Slug) -> Self {
        Self(slug)
    }
}

impl fmt::Display for Anchor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}
