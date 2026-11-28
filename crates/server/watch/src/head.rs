//! Where a repository is now.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

use build::{Branch, Revision, RevisionError};
use model::git::Locator;

use crate::Access;

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

/// A branch of a repository and the commit it is at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tip {
    /// The branch.
    pub branch: Branch,
    /// Its commit.
    pub head: Head,
}

/// A reference the remote advertised under `refs/heads/` whose name is no branch this reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// The name as the remote wrote it.
    pub written: String,
    /// Why it is no branch.
    pub source: RevisionError,
}

/// The branches of a repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Branches {
    /// The branch `HEAD` points at, when the remote says.
    pub default: Option<Branch>,
    /// Each branch with a commit, by name.
    pub found: Vec<Tip>,
    /// The references that are no branch to this.
    pub skipped: Vec<Skipped>,
}

/// Where a repository's branch is. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Worker's fetch is not Send")]
pub trait Heads {
    /// The adapter's own error.
    type Error: Error + 'static;

    /// The commit `revision` names at `url` now, or the remote's default branch when none is
    /// named, read with `access` when the site has a credential. It reads the remote's list of
    /// references and fetches no objects.
    async fn head(
        &self,
        url: &Locator,
        revision: Option<&Revision>,
        access: Option<&Access>,
    ) -> Result<Head, Self::Error>;

    /// Every branch of the repository at `url` and its commit, read with `access` when the site
    /// has a credential. Like [`Heads::head`] it fetches no objects.
    async fn branches(&self, url: &Locator, access: Option<&Access>) -> Result<Branches, Self::Error>;
}
