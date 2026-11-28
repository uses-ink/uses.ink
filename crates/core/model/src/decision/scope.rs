//! The kinds of thing a decision can be about, as types, so a code fixes its subject kind.

use std::fmt;
use std::hash::Hash;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ContentPath;
use crate::code::AnyCode;

/// A kind of subject: [`Site`] or [`File`]. Each converts to and from the erased
/// [`crate::Subject`], refusing the other kind.
pub trait Subject:
    Clone
    + Ord
    + Hash
    + fmt::Debug
    + fmt::Display
    + Serialize
    + DeserializeOwned
    + Into<crate::Subject>
    + TryFrom<crate::Subject, Error = Mismatch>
{
    /// Which kind it is.
    const SCOPE: Scope;
}

/// Whether a decision is made once for the site or once per file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// Once for the site.
    Site,
    /// Once per file.
    File,
}

impl fmt::Display for Scope {
    /// `the site`, `a file`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Site => "the site",
            Self::File => "a file",
        })
    }
}

/// The site as a subject. Written as the erased [`crate::Subject::Site`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "crate::Subject", try_from = "crate::Subject")]
pub struct Site;

impl Subject for Site {
    const SCOPE: Scope = Scope::Site;
}

impl From<Site> for crate::Subject {
    fn from(_: Site) -> Self {
        Self::Site
    }
}

impl TryFrom<crate::Subject> for Site {
    type Error = Mismatch;

    fn try_from(subject: crate::Subject) -> Result<Self, Self::Error> {
        match subject {
            crate::Subject::Site => Ok(Self),
            other => Err(Mismatch::Scope { expected: Scope::Site, found: other.scope() }),
        }
    }
}

impl fmt::Display for Site {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the site")
    }
}

/// One file as a subject. Written as the erased [`crate::Subject::File`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "crate::Subject", try_from = "crate::Subject")]
pub struct File(pub ContentPath);

impl Subject for File {
    const SCOPE: Scope = Scope::File;
}

impl From<ContentPath> for File {
    fn from(path: ContentPath) -> Self {
        Self(path)
    }
}

impl AsRef<ContentPath> for File {
    fn as_ref(&self) -> &ContentPath {
        &self.0
    }
}

impl From<File> for crate::Subject {
    fn from(file: File) -> Self {
        Self::File(file.0)
    }
}

impl TryFrom<crate::Subject> for File {
    type Error = Mismatch;

    fn try_from(subject: crate::Subject) -> Result<Self, Self::Error> {
        match subject {
            crate::Subject::File(path) => Ok(Self(path)),
            other => Err(Mismatch::Scope { expected: Scope::File, found: other.scope() }),
        }
    }
}

impl fmt::Display for File {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, out)
    }
}

/// An erased decision or subject that is not of the kind a typed one needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mismatch {
    /// A subject of the other scope.
    Scope {
        /// The scope the code decides on.
        expected: Scope,
        /// The scope the subject has.
        found: Scope,
    },
    /// A decision of another code.
    Code {
        /// The code wanted.
        expected: AnyCode,
        /// The code it has.
        found: AnyCode,
    },
}

impl fmt::Display for Mismatch {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope { expected, found } => write!(out, "the subject is {found}, not {expected}"),
            Self::Code { expected, found } => write!(out, "the decision is `{found}`, not `{expected}`"),
        }
    }
}

impl std::error::Error for Mismatch {}
