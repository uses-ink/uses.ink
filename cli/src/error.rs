//! What stops a command before any diagnostic could: the folder or repository refused, an output
//! folder `ink` will not write into, a terminal that cannot be written to.

use std::fmt;
use std::path::PathBuf;

use crate::exit::Exit;

/// A command that could not run to its end.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The folder could not be read as a snapshot: reported as its diagnostic.
    Refused(host_native::Refused),
    /// `--rev` was given for a folder read as the files on disk.
    Unversioned {
        /// The site's folder.
        dir: PathBuf,
    },
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
    /// The path given to `ink explain` is no file of the folder.
    Unknown {
        /// The path.
        path: model::ContentPath,
    },
    /// The output folder is one `ink build` will not clear or write into.
    Output(crate::out::Refusal),
    /// An entry of the deploy has no place in the output folder.
    Placed {
        /// Where it would go, as built from its route.
        file: String,
        /// Why that is no place.
        source: crate::out::RelativeError,
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
    /// A component `ink build --check` runs could not be read.
    Component {
        /// Its file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A component could not be loaded.
    Plugin {
        /// Why.
        source: host_wasmtime::Error,
    },
    /// A result could not be written as JSON.
    Json {
        /// Why it could not.
        source: serde_json::Error,
    },
}

impl Error {
    /// The exit code it ends the command with: a site's folder that is missing or cannot be
    /// listed is a usage error, like any path given wrong; what is wrong inside it is content.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Refused(
                host_native::Refused::Root { .. }
                | host_native::Refused::Repository { .. }
                | host_native::Refused::Revision { .. },
            ) => Exit::Usage,
            Self::Refused(_) => Exit::Content,
            _ => Exit::Usage,
        }
    }

    /// What kind of error it is, as `--json` names it: a refused folder's diagnostic code, or
    /// `noun.reason` for the rest.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Refused(refused) => model::Diagnostic::from(refused).code(),
            Self::Unversioned { .. } => "source.rev.unversioned",
            Self::Base { .. } => "base.address.invalid",
            Self::Path { .. } => "explain.path.invalid",
            Self::Unknown { .. } => "explain.path.unknown",
            Self::Output(refusal) => refusal.kind(),
            Self::Placed { .. } => "output.entry.unplaced",
            Self::Blob { .. } => "output.blob.missing",
            Self::Read { .. } => "output.file.unreadable",
            Self::Write { .. } => "output.file.unwritable",
            Self::Manifest { .. } => "output.manifest.invalid",
            Self::Serve { .. } => "preview.serve.failed",
            Self::Terminal { .. } => "cli.terminal.unwritable",
            Self::Component { .. } => "component.file.unreadable",
            Self::Plugin { .. } => "component.load.failed",
            Self::Json { .. } => "cli.json.unwritable",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(out, "{refused}"),
            Self::Unversioned { dir } => {
                write!(out, "`--rev` names a commit, and {} is read as a folder, not a git repository", dir.display())
            }
            Self::Base { written, .. } => write!(out, "`{written}` is not an address a site is served at"),
            Self::Path { written, .. } => write!(out, "`{written}` is not a path in the folder"),
            Self::Unknown { path } => write!(out, "{path} is not a file of the folder"),
            Self::Output(refusal) => write!(out, "{refusal}"),
            Self::Placed { file, .. } => write!(out, "`{file}` is not a file `ink build` can write"),
            Self::Blob { route, .. } => write!(out, "the bytes of {route} are missing"),
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Write { path, .. } => write!(out, "{} could not be written", path.display()),
            Self::Manifest { path, .. } => {
                write!(out, "the earlier build's manifest {} does not parse", path.display())
            }
            Self::Serve { .. } => write!(out, "the preview could not be served"),
            Self::Terminal { .. } => write!(out, "the terminal could not be written to"),
            Self::Component { path, .. } => write!(out, "the component {} could not be read", path.display()),
            Self::Plugin { .. } => write!(out, "a component could not be loaded"),
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
            Self::Write { source, .. }
            | Self::Read { source, .. }
            | Self::Serve { source }
            | Self::Terminal { source }
            | Self::Component { source, .. } => Some(source),
            Self::Plugin { source } => Some(source),
            Self::Placed { source, .. } => Some(source),
            Self::Blob { source, .. } => Some(source),
            Self::Manifest { source, .. } | Self::Json { source } => Some(source),
            Self::Output(_) | Self::Unknown { .. } | Self::Unversioned { .. } => None,
        }
    }
}

impl From<std::io::Error> for Error {
    /// A failed write to stdout or stderr.
    fn from(source: std::io::Error) -> Self {
        Self::Terminal { source }
    }
}
