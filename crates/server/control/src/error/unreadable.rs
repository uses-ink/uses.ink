//! Stored rows that do not read back.

use std::fmt;

use crate::HandleError;
use crate::unreadable::Unreadable as Cell;

/// A stored row that does not read back as the domain value it should be: the database was
/// changed by something other than this code, or by another version of it.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unreadable {
    /// A column that is not what any row holds ([`Cell`]).
    Cell(Cell),
    /// The column holds a bad handle.
    Handle {
        /// Why.
        source: HandleError,
    },
    /// The column holds a bad preview label.
    Slug {
        /// Why.
        source: crate::SlugError,
    },
    /// The column holds a bad build id.
    Build {
        /// Why.
        source: build::BuildIdError,
    },
    /// The column holds a kind of source this version does not know.
    Locator {
        /// The column.
        column: &'static str,
    },
    /// The column holds a repository address that is no longer valid.
    Url {
        /// The column.
        column: &'static str,
        /// Why.
        source: model::git::LocatorError,
    },
    /// The column holds a revision that is no longer valid.
    Revision {
        /// The column.
        column: &'static str,
        /// Why.
        source: build::RevisionError,
    },
    /// The `seen` table holds a commit id that is no commit id.
    Commit {
        /// Why.
        source: model::VersionError,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cell(cell) => cell.fmt(out),
            Self::Handle { .. } => out.write_str("column `handle` holds a bad handle"),
            Self::Slug { .. } => out.write_str("column `slug` holds a bad preview label"),
            Self::Build { .. } => out.write_str("a stored build id is bad"),
            Self::Locator { column } => {
                write!(out, "column `{column}` holds a kind of source this version does not know")
            }
            Self::Url { column, .. } => {
                write!(out, "column `{column}` holds a repository address that is no longer valid")
            }
            Self::Revision { column, .. } => write!(out, "column `{column}` holds a revision that is no longer valid"),
            Self::Commit { .. } => out.write_str("a stored commit id is bad"),
        }
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cell(source) => source.source(),
            Self::Handle { source } => Some(source),
            Self::Slug { source } => Some(source),
            Self::Build { source } => Some(source),
            Self::Url { source, .. } => Some(source),
            Self::Revision { source, .. } => Some(source),
            Self::Commit { source } => Some(source),
            Self::Locator { .. } => None,
        }
    }
}

impl From<Cell> for Unreadable {
    fn from(source: Cell) -> Self {
        Self::Cell(source)
    }
}

impl From<db::ColumnError> for Unreadable {
    fn from(source: db::ColumnError) -> Self {
        Self::Cell(source.into())
    }
}
