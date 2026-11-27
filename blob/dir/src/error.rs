//! What the directory adapter fails with.

use std::fmt;
use std::path::PathBuf;

/// A failure of the directory adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A folder could not be made.
    Create {
        /// The folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A file could not be written, synced or moved into place.
    Write {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A file or folder could not be read.
    Read {
        /// The path.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A blob could not be removed.
    Remove {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// Something in a tenant's folder is not a blob: a name that is not a hash, or an entry that
    /// is not a plain file.
    Stray {
        /// The entry.
        path: PathBuf,
    },
    /// A write failed, and so did removing its half-written file.
    Discard {
        /// The half-written file that stays.
        path: PathBuf,
        /// Why it could not be removed.
        source: std::io::Error,
        /// The failure the write had.
        cause: Box<Error>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create { path, .. } => write!(out, "the folder {} could not be made", path.display()),
            Self::Write { path, .. } => write!(out, "{} could not be written", path.display()),
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Remove { path, .. } => write!(out, "{} could not be removed", path.display()),
            Self::Stray { path } => write!(out, "{} is in the blob store and is not a blob", path.display()),
            Self::Discard { path, .. } => write!(out, "a failed write left {} behind", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Create { source, .. }
            | Self::Write { source, .. }
            | Self::Read { source, .. }
            | Self::Remove { source, .. } => Some(source),
            Self::Discard { cause, .. } => Some(cause.as_ref()),
            Self::Stray { .. } => None,
        }
    }
}
