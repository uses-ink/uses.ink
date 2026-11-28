//! The output folder, and the files read and written beside it.

use std::fmt;
use std::path::PathBuf;

/// A folder or file `ink` reads or writes that it could not.
#[derive(Debug)]
#[non_exhaustive]
pub enum FolderError {
    /// The output folder is one `ink build` will not clear or write into.
    Output(crate::folder::written::Refusal),
    /// An entry of the deploy has no place in the output folder.
    Placed {
        /// Where it would go, as built from its route.
        file: String,
        /// Why that is no place.
        source: crate::folder::written::RelativeError,
    },
    /// An entry's bytes are in no store.
    Blob {
        /// The entry's route.
        route: model::Route,
        /// Which blob is missing.
        source: engine::BlobMissing,
    },
    /// A file or folder in the output folder could not be read.
    Read {
        /// The file or folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The output folder or a file in it could not be written.
    Write {
        /// The folder or file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The previous build's manifest in the output folder does not parse.
    Manifest {
        /// The manifest's file.
        path: PathBuf,
        /// Why it does not parse.
        source: serde_json::Error,
    },
}

impl FolderError {
    /// What kind of error it is, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Output(refusal) => refusal.kind(),
            Self::Placed { .. } => "output.entry.unplaced",
            Self::Blob { .. } => "output.blob.missing",
            Self::Read { .. } => "output.file.unreadable",
            Self::Write { .. } => "output.file.unwritable",
            Self::Manifest { .. } => "output.manifest.invalid",
        }
    }
}

impl fmt::Display for FolderError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Output(refusal) => write!(out, "{refusal}"),
            Self::Placed { file, .. } => write!(out, "`{file}` is not a file `ink build` can write"),
            Self::Blob { route, .. } => write!(out, "the bytes of {route} are missing"),
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Write { path, .. } => write!(out, "{} could not be written", path.display()),
            Self::Manifest { path, .. } => {
                write!(out, "the earlier build's manifest {} does not parse", path.display())
            }
        }
    }
}

impl std::error::Error for FolderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write { source, .. } | Self::Read { source, .. } => Some(source),
            Self::Placed { source, .. } => Some(source),
            Self::Blob { source, .. } => Some(source),
            Self::Manifest { source, .. } => Some(source),
            Self::Output(_) => None,
        }
    }
}
