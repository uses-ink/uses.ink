//! The folder or repository a command reads, refused, and the options that say how to read it.

use std::fmt;
use std::path::PathBuf;

use crate::output::exit::Exit;

/// What was wrong with the source a command was asked to read.
#[derive(Debug)]
#[non_exhaustive]
pub enum SourceError {
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
    /// `site.format` names a format plugin this ink does not hold.
    Format {
        /// The id.
        id: model::record::PluginId,
    },
}

impl SourceError {
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

    /// What kind of error it is, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Refused(refused) => model::Diagnostic::from(refused).code(),
            Self::Unversioned { .. } => "source.rev.unversioned",
            Self::Base { .. } => "base.address.invalid",
            Self::Format { .. } => "format.plugin.unknown",
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(out, "{refused}"),
            Self::Unversioned { dir } => {
                write!(out, "`--rev` names a commit, and {} is read as a folder, not a git repository", dir.display())
            }
            Self::Base { written, .. } => write!(out, "`{written}` is not an address a site is served at"),
            Self::Format { id } => write!(out, "`site.format` names `{id}`, which this ink does not hold"),
        }
    }
}

impl std::error::Error for SourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(source) => Some(source),
            Self::Base { source, .. } => Some(source),
            Self::Unversioned { .. } | Self::Format { .. } => None,
        }
    }
}
