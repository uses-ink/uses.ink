//! Why a folder was not read. Each refusal becomes a [`Diagnostic`] with a catalog code at the
//! boundary.

use std::fmt;
use std::path::PathBuf;

use model::{Bytes, ContentPath, Diagnostic, PathError, Problem, SnapshotError, Subject};

use crate::blob::StoreError;

/// A folder that could not be read as a snapshot. Nothing is read in part: a refusal stops the
/// read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Refused {
    /// The folder itself could not be opened.
    Root {
        /// The folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A file or folder inside could not be read.
    Unreadable {
        /// The file or folder.
        path: ContentPath,
        /// What the system said.
        source: std::io::Error,
    },
    /// A name that is not UTF-8.
    Encoding {
        /// The path, as the system gives it.
        path: PathBuf,
    },
    /// A UTF-8 name that is no [`ContentPath`], such as one holding `\`.
    Name {
        /// The name.
        written: String,
        /// Why it is no path.
        source: PathError,
    },
    /// A link whose target is outside the folder.
    Outside {
        /// The link.
        path: ContentPath,
        /// Where it points.
        target: PathBuf,
    },
    /// More folders than the budget. The walk stops at the first folder past it.
    Folders {
        /// How many folders a site may hold.
        limit: u64,
    },
    /// Neither a file nor a folder: a socket, a pipe, a device.
    Special {
        /// The path.
        path: ContentPath,
    },
    /// More files than the budget. The walk stops at the first file past it.
    Files {
        /// How many files the walk met before it stopped.
        count: u64,
        /// How many a site may hold.
        limit: u64,
    },
    /// One file larger than the budget.
    File {
        /// The file.
        path: ContentPath,
        /// How large it is.
        size: Bytes,
        /// How large a file may be.
        limit: Bytes,
    },
    /// All files together larger than the budget. The walk stops at the first file past it.
    Total {
        /// How many bytes the walk read before it stopped, each file once.
        size: Bytes,
        /// How large a site may be.
        limit: Bytes,
    },
    /// A file's bytes could not be stored.
    Store(StoreError),
    /// The files do not make a snapshot.
    Snapshot(SnapshotError),
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root { path, .. } => write!(out, "the folder {} could not be read", path.display()),
            Self::Unreadable { path, .. } => write!(out, "{path} could not be read"),
            Self::Encoding { path } => write!(out, "the name of {} is not UTF-8", path.display()),
            Self::Name { written, .. } => write!(out, "`{written}` is not a name a site can hold"),
            Self::Outside { path, target } => write!(out, "{path} links outside the folder, to {}", target.display()),
            Self::Folders { limit } => {
                write!(out, "the folder holds more than {limit} folders, the most a site may hold")
            }
            Self::Special { path } => write!(out, "{path} is neither a file nor a folder"),
            Self::Files { limit, .. } => {
                write!(out, "the folder holds more than {limit} files, the most a site may hold")
            }
            Self::File { path, size, limit } => write!(out, "{path} is {size}; a file may be {limit}"),
            Self::Total { size, limit } => write!(out, "the folder holds at least {size}; a site may hold {limit}"),
            Self::Store(_) => write!(out, "a file's bytes could not be stored"),
            Self::Snapshot(_) => write!(out, "the files do not make a snapshot"),
        }
    }
}

impl std::error::Error for Refused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Root { source, .. } | Self::Unreadable { source, .. } => Some(source),
            Self::Name { source, .. } => Some(source),
            Self::Store(source) => Some(source),
            Self::Snapshot(source) => Some(source),
            Self::Encoding { .. }
            | Self::Outside { .. }
            | Self::Folders { .. }
            | Self::Special { .. }
            | Self::Files { .. }
            | Self::File { .. }
            | Self::Total { .. } => None,
        }
    }
}

impl From<&Refused> for Diagnostic {
    /// The refusal as the author or operator sees it. An I/O cause is erased to its text here,
    /// at the edge of the host: the catalog's `reason` field.
    fn from(refused: &Refused) -> Self {
        let on = |path: &ContentPath| Subject::File(path.clone());
        match refused {
            Refused::Root { source, .. } => Self::new(Problem::SourceRootUnreadable { reason: source.to_string() }),
            Refused::Unreadable { path, source } => {
                Self::new(Problem::SourceFileUnreadable { path: path.clone(), reason: source.to_string() }).on(on(path))
            }
            Refused::Encoding { path } => {
                Self::new(Problem::SourceNameInvalid { written: path.to_string_lossy().into_owned() })
            }
            Refused::Name { written, .. } => Self::new(Problem::SourceNameInvalid { written: written.clone() }),
            Refused::Outside { path, .. } => Self::new(Problem::SourceLinkOutside { path: path.clone() }).on(on(path)),
            Refused::Folders { limit } => Self::new(Problem::SourceBudgetFolders { limit: *limit }),
            Refused::Special { path } => Self::new(Problem::SourceFileSpecial { path: path.clone() }).on(on(path)),
            Refused::Files { count, limit } => Self::new(Problem::SourceBudgetFiles { count: *count, limit: *limit }),
            Refused::File { path, size, limit } => {
                Self::new(Problem::SourceBudgetFile { path: path.clone(), size: *size, limit: *limit }).on(on(path))
            }
            Refused::Total { size, limit } => Self::new(Problem::SourceBudgetBytes { size: *size, limit: *limit }),
            Refused::Store(error) => Self::new(Problem::SourceBlobUnwritable { reason: error.to_string() }),
            Refused::Snapshot(error) => Self::new(Problem::SourceRootUnreadable { reason: error.to_string() }),
        }
    }
}
