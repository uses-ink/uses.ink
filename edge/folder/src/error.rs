//! Why a folder is not served.

use std::fmt;
use std::path::PathBuf;

use model::{DeployId, DuplicateRoute, PathError, Route};

/// A folder that cannot be served, or a file that cannot be read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The folder given as one deploy holds no `.ink-manifest.json`.
    NotDeploy {
        /// The folder.
        root: PathBuf,
    },
    /// The folder given holds deploys by host and is no folder.
    NotFolder {
        /// The path.
        root: PathBuf,
    },
    /// A file or folder could not be read.
    Read {
        /// Where.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The manifest does not parse.
    Manifest {
        /// The manifest.
        path: PathBuf,
        /// Why.
        source: serde_json::Error,
    },
    /// The manifest is of a schema this edge does not read.
    Schema {
        /// The manifest.
        path: PathBuf,
        /// The schema it names.
        found: String,
    },
    /// The deploy id the manifest records is not the hash of the entries it lists.
    Id {
        /// The manifest.
        path: PathBuf,
        /// The id it records.
        recorded: DeployId,
        /// The id its entries hash to.
        computed: DeployId,
    },
    /// The manifest lists a route twice.
    Duplicate {
        /// The manifest.
        path: PathBuf,
        /// Which.
        source: DuplicateRoute,
    },
    /// A file the manifest lists is not a path inside the folder.
    Path {
        /// The manifest.
        path: PathBuf,
        /// Why.
        source: PathError,
    },
    /// A file the manifest lists has a part that is not a plain name.
    Escapes {
        /// The manifest.
        path: PathBuf,
    },
    /// An entry's blob is in no file the manifest lists.
    Unplaced {
        /// The route.
        route: Route,
    },
    /// A link where a plain file or folder is required.
    Linked {
        /// Where.
        path: PathBuf,
    },
    /// Something that is not a plain file where a file is required.
    NotFile {
        /// Where.
        path: PathBuf,
    },
    /// A file ended before the bytes asked for: it changed since the manifest was read.
    Short {
        /// The file.
        path: PathBuf,
        /// How many bytes were still owed.
        owed: u64,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDeploy { root } => {
                write!(out, "{} holds no `.ink-manifest.json`: run `ink build` into it", root.display())
            }
            Self::NotFolder { root } => write!(out, "{} is not a folder", root.display()),
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Manifest { path, .. } => write!(out, "the manifest {} does not parse", path.display()),
            Self::Schema { path, found } => {
                write!(out, "the manifest {} is of schema {found:?}, not uses.manifest/v3", path.display())
            }
            Self::Id { path, recorded, computed } => {
                write!(
                    out,
                    "the manifest {} records deploy {recorded} but its entries hash to {computed}",
                    path.display()
                )
            }
            Self::Duplicate { path, .. } => write!(out, "the manifest {} lists a route twice", path.display()),
            Self::Path { path, .. } => {
                write!(out, "the manifest {} lists a file that is not a path in its folder", path.display())
            }
            Self::Escapes { path } => {
                write!(out, "the manifest {} lists a file with a part that leaves its folder", path.display())
            }
            Self::Unplaced { route } => {
                write!(out, "the manifest serves {route} from a blob that no listed file holds")
            }
            Self::Linked { path } => write!(out, "{} is a link, which is never followed", path.display()),
            Self::NotFile { path } => write!(out, "{} is not a plain file", path.display()),
            Self::Short { path, owed } => {
                write!(out, "{} ended {owed} bytes early: it changed since the manifest was read", path.display())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Manifest { source, .. } => Some(source),
            Self::Duplicate { source, .. } => Some(source),
            Self::Path { source, .. } => Some(source),
            _ => None,
        }
    }
}
