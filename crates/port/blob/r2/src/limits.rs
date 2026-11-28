//! What the adapter is told about R2's limits, and what it refuses.

use std::fmt;

/// The numbers the adapter works to. Both are configurable, and the defaults are R2's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    part: usize,
    page: u32,
}

impl Limits {
    /// The smallest part R2 takes in a multipart upload, except the last: 5 MiB.
    pub const LEAST: usize = 5 << 20;

    /// The most parts R2 takes in one upload.
    pub const PARTS: usize = 10_000;

    /// The most keys R2 lists in a page.
    pub const PAGE: u32 = 1_000;

    /// The part size the adapter uses unless told another: 8 MiB, as measured in decision 0096.
    pub const PART: usize = 8 << 20;

    /// Limits with parts of `part` bytes and listing pages of `page` keys.
    pub const fn new(part: usize, page: u32) -> Result<Self, Refused> {
        if part < Self::LEAST {
            return Err(Refused::Part { given: part });
        }
        if page == 0 || page > Self::PAGE {
            return Err(Refused::Page { given: page });
        }
        Ok(Self { part, page })
    }

    /// The bytes of one part: the most the adapter holds of a blob at once.
    pub const fn part(&self) -> usize {
        self.part
    }

    /// The keys one listing call asks for.
    pub const fn page(&self) -> u32 {
        self.page
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self { part: Self::PART, page: Self::PAGE }
    }
}

/// Limits R2 would not take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// A part smaller than R2's least.
    Part {
        /// What was given.
        given: usize,
    },
    /// A page of no keys, or of more than R2 lists.
    Page {
        /// What was given.
        given: u32,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Part { given } => write!(out, "a part of {given} bytes is under R2's least of {}", Limits::LEAST),
            Self::Page { given } => write!(out, "a page of {given} keys is not between 1 and {}", Limits::PAGE),
        }
    }
}

impl std::error::Error for Refused {}
