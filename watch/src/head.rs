//! Where a repository is now.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use build::Revision;
use model::ExternalUrl;

/// A commit as the remote names it: 40 or 64 lower-case hex digits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Head(String);

impl Head {
    /// The commit as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Head {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Text that is no commit id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HeadError {
    /// Not 40 or 64 characters.
    Length {
        /// How long.
        found: usize,
    },
    /// Not a lower-case hex digit, at this offset.
    Digit {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for HeadError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a commit id is 40 or 64 hex digits, not {found}"),
            Self::Digit { at } => write!(out, "byte {at} of the commit id is not a lower-case hex digit"),
        }
    }
}

impl Error for HeadError {}

impl FromStr for Head {
    type Err = HeadError;

    fn from_str(text: &str) -> Result<Self, HeadError> {
        if text.len() != 40 && text.len() != 64 {
            return Err(HeadError::Length { found: text.len() });
        }
        match text.bytes().position(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
            Some(at) => Err(HeadError::Digit { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

/// Where a repository's branch is. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Worker's fetch is not Send")]
pub trait Heads {
    /// The adapter's own error.
    type Error: Error + 'static;

    /// The commit `revision` names at `url` now, or the remote's default branch when none is
    /// named. It reads the remote's list of references and fetches no objects.
    async fn head(&self, url: &ExternalUrl, revision: Option<&Revision>) -> Result<Head, Self::Error>;
}
