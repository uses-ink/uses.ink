//! What a build reads.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use model::Hash;
use model::git::Locator;
use serde::Serialize;

/// A git revision as the user wrote it: a branch, tag or commit. Not empty, with no whitespace
/// or control characters, and it does not start with `-`, so it can never be read as an option.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Revision(String);

impl Revision {
    /// The revision as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Revision {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Text that is no revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RevisionError {
    /// Empty.
    Empty,
    /// Starts with a hyphen.
    Option,
    /// Whitespace or a control character at this byte offset.
    Character {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for RevisionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a revision is not empty"),
            Self::Option => out.write_str("a revision does not start with a hyphen"),
            Self::Character { at } => write!(out, "whitespace or a control character at byte {at} of the revision"),
        }
    }
}

impl std::error::Error for RevisionError {}

impl FromStr for Revision {
    type Err = RevisionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(RevisionError::Empty);
        }
        if text.starts_with('-') {
            return Err(RevisionError::Option);
        }
        match text.char_indices().find(|(_, c)| c.is_whitespace() || c.is_control()) {
            Some((at, _)) => Err(RevisionError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

/// The kind of a [`Source`], by the name it is stored under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Kind {
    /// A folder on the builder's disk.
    Folder,
    /// A git repository.
    Git,
    /// Built output that is sent to the control plane, so nothing is fetched.
    Upload,
}

impl fmt::Display for Kind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Folder => "folder",
            Self::Git => "git",
            Self::Upload => "upload",
        })
    }
}

/// Where a build's content comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Source {
    /// A folder on the builder's disk.
    Folder(PathBuf),
    /// A git repository, at a revision or its default branch: over HTTPS, or on the builder's own
    /// disk when the operator allows it.
    Git {
        /// Where to fetch it.
        url: Locator,
        /// What to build, or the remote's default.
        revision: Option<Revision>,
    },
    /// Built output sent to the control plane (`PUT` of an archive): no builder reads it, and the
    /// poll does not look at it.
    Upload,
    /// A tree sent to the control plane (`ink push`): the tree is stored as a blob of the site's
    /// tenant under this hash, and so is every file it lists. Only a build's request holds this;
    /// a site's source is [`Source::Upload`].
    Bundle(Hash),
}

impl Source {
    /// Which kind of source this is.
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Folder(_) => Kind::Folder,
            Self::Git { .. } => Kind::Git,
            Self::Upload | Self::Bundle(_) => Kind::Upload,
        }
    }
}
