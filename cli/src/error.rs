//! What stops a command before any diagnostic could: the folder refused, an output folder `ink`
//! will not write into, a terminal that cannot be written to.

use std::fmt;
use std::path::PathBuf;

use crate::exit::Exit;

/// A command that could not run to its end.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The folder could not be read as a snapshot: reported as its diagnostic.
    Refused(host_native::Refused),
    /// `--base` is no site address.
    Base {
        /// The address, as given.
        written: String,
        /// Why it is no address.
        source: engine::BaseUrlError,
    },
    /// The path given to `ink explain` is no path of a site.
    Path {
        /// The path, as given.
        written: String,
        /// Why it is no path.
        source: model::PathError,
    },
    /// The output folder holds files that no earlier `ink build` wrote.
    Foreign {
        /// The output folder.
        path: PathBuf,
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
    /// The preview server could not listen or serve.
    Serve {
        /// What the system said.
        source: std::io::Error,
    },
    /// Stdout or stderr could not be written.
    Terminal {
        /// What the system said.
        source: std::io::Error,
    },
    /// A result could not be written as JSON.
    Json {
        /// Why it could not.
        source: serde_json::Error,
    },
}

impl Error {
    /// The exit code it ends the command with.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Refused(_) => Exit::Content,
            _ => Exit::Usage,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(out, "{refused}"),
            Self::Base { written, .. } => write!(out, "`{written}` is not an address a site is served at"),
            Self::Path { written, .. } => write!(out, "`{written}` is not a path in the folder"),
            Self::Foreign { path } => {
                write!(out, "{} holds files no `ink build` wrote; choose an empty folder with `-o`", path.display())
            }
            Self::Write { path, .. } => write!(out, "{} could not be written", path.display()),
            Self::Manifest { path, .. } => {
                write!(out, "the earlier build's manifest {} does not parse", path.display())
            }
            Self::Serve { .. } => write!(out, "the preview could not be served"),
            Self::Terminal { .. } => write!(out, "the terminal could not be written to"),
            Self::Json { .. } => write!(out, "the result could not be written as JSON"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(source) => Some(source),
            Self::Base { source, .. } => Some(source),
            Self::Path { source, .. } => Some(source),
            Self::Write { source, .. } | Self::Serve { source } | Self::Terminal { source } => Some(source),
            Self::Manifest { source, .. } | Self::Json { source } => Some(source),
            Self::Foreign { .. } => None,
        }
    }
}

impl From<std::io::Error> for Error {
    /// A failed write to stdout or stderr.
    fn from(source: std::io::Error) -> Self {
        Self::Terminal { source }
    }
}
