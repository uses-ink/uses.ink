//! Byte ranges in a file.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A range of bytes in one file: `start` inclusive, `end` exclusive, `start <= end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "(usize, usize)", into = "(usize, usize)")]
pub struct Span {
    start: usize,
    end: usize,
}

/// A span whose end comes before its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backwards {
    /// Where it starts.
    pub start: usize,
    /// Where it ends, before `start`.
    pub end: usize,
}

impl fmt::Display for Backwards {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "a span ends at {} before it starts at {}", self.end, self.start)
    }
}

impl std::error::Error for Backwards {}

impl Span {
    /// The bytes from `start` up to `end`, when `start <= end`.
    pub const fn new(start: usize, end: usize) -> Result<Self, Backwards> {
        if start <= end { Ok(Self { start, end }) } else { Err(Backwards { start, end }) }
    }

    /// The first byte.
    pub const fn start(self) -> usize {
        self.start
    }

    /// The byte after the last.
    pub const fn end(self) -> usize {
        self.end
    }

    /// How many bytes it covers.
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    /// Whether it covers no bytes.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// The range, for slicing with `str::get`.
    pub const fn range(self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

impl TryFrom<(usize, usize)> for Span {
    type Error = Backwards;

    fn try_from((start, end): (usize, usize)) -> Result<Self, Self::Error> {
        Self::new(start, end)
    }
}

impl From<Span> for (usize, usize) {
    fn from(span: Span) -> Self {
        (span.start, span.end)
    }
}

impl fmt::Display for Span {
    /// `bytes 12..18`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "bytes {}..{}", self.start, self.end)
    }
}
