//! What stops a command before any diagnostic could: the folder or repository refused, an output
//! folder `ink` will not write into, a terminal that cannot be written to. The variants are
//! grouped by origin, one module each; each keeps its fields, and `Error` shows its origin's text
//! and causes as its own.

mod ask;
mod component;
mod edit;
mod folder;
mod serve;
mod source;

use std::fmt;

use crate::output::exit::Exit;

pub(crate) use ask::AskError;
pub(crate) use component::ComponentError;
pub(crate) use edit::EditError;
pub(crate) use folder::FolderError;
pub(crate) use serve::ServeError;
pub(crate) use source::SourceError;

/// A command that could not run to its end.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The folder or repository to read, or how to read it.
    Source(SourceError),
    /// A path, code or pattern given on the command line.
    Ask(AskError),
    /// A change to `ink.toml` or a page that could not be made.
    Edit(EditError),
    /// The output folder, or a file read or written beside it.
    Folder(FolderError),
    /// The preview or the server.
    Serve(ServeError),
    /// A component `ink build --check` runs.
    Component(ComponentError),
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
    /// A remote command (`ink deploy`) did not do its work.
    Deploy {
        /// Why.
        source: crate::deploy::Failure,
    },
    /// The user's config file could not be read.
    Preferences {
        /// Why.
        source: crate::settings::Unreadable,
    },
    /// `ink help` has nothing to say about the words.
    Help {
        /// Why.
        source: crate::grammar::help::Refusal,
    },
    /// The manual page could not be written.
    Manual {
        /// What the system said.
        source: std::io::Error,
    },
}

impl Error {
    /// The exit code it ends the command with: a site's folder that is missing or cannot be
    /// listed is a usage error, like any path given wrong; what is wrong inside it is content.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Source(source) => source.exit(),
            Self::Deploy { source } => source.exit(),
            _ => Exit::Usage,
        }
    }

    /// What kind of error it is, as `--json` names it: a refused folder's diagnostic code, or
    /// `noun.reason` for the rest.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Source(source) => source.kind(),
            Self::Ask(source) => source.kind(),
            Self::Edit(source) => source.kind(),
            Self::Folder(source) => source.kind(),
            Self::Serve(source) => source.kind(),
            Self::Component(source) => source.kind(),
            Self::Terminal { .. } => "cli.terminal.unwritable",
            Self::Json { .. } => "cli.json.unwritable",
            Self::Deploy { source } => source.kind(),
            Self::Preferences { .. } => "config.file.unreadable",
            Self::Help { .. } => "help.topic.unknown",
            Self::Manual { .. } => "cli.manual.unwritable",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(source) => write!(out, "{source}"),
            Self::Ask(source) => write!(out, "{source}"),
            Self::Edit(source) => write!(out, "{source}"),
            Self::Folder(source) => write!(out, "{source}"),
            Self::Serve(source) => write!(out, "{source}"),
            Self::Component(source) => write!(out, "{source}"),
            Self::Terminal { .. } => write!(out, "the terminal could not be written to"),
            Self::Json { .. } => write!(out, "the result could not be written as JSON"),
            Self::Deploy { source } => write!(out, "{source}"),
            Self::Preferences { source } => write!(out, "{source}"),
            Self::Help { source } => write!(out, "{source}"),
            Self::Manual { .. } => write!(out, "the manual page could not be written"),
        }
    }
}

/// The origin's own error is not a cause of its own: its text is `Error`'s, and its source is the
/// first cause.
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(source) => std::error::Error::source(source),
            Self::Ask(source) => std::error::Error::source(source),
            Self::Edit(source) => std::error::Error::source(source),
            Self::Folder(source) => std::error::Error::source(source),
            Self::Serve(source) => std::error::Error::source(source),
            Self::Component(source) => std::error::Error::source(source),
            Self::Terminal { source } | Self::Manual { source } => Some(source),
            Self::Json { source } => Some(source),
            Self::Deploy { source } => source.source(),
            Self::Preferences { source } => source.source(),
            Self::Help { source } => source.source(),
        }
    }
}

impl From<crate::deploy::Failure> for Error {
    fn from(source: crate::deploy::Failure) -> Self {
        Self::Deploy { source }
    }
}

impl From<std::io::Error> for Error {
    /// A failed write to stdout or stderr.
    fn from(source: std::io::Error) -> Self {
        Self::Terminal { source }
    }
}

impl From<SourceError> for Error {
    fn from(source: SourceError) -> Self {
        Self::Source(source)
    }
}

impl From<AskError> for Error {
    fn from(source: AskError) -> Self {
        Self::Ask(source)
    }
}

impl From<EditError> for Error {
    fn from(source: EditError) -> Self {
        Self::Edit(source)
    }
}

impl From<FolderError> for Error {
    fn from(source: FolderError) -> Self {
        Self::Folder(source)
    }
}

impl From<ServeError> for Error {
    fn from(source: ServeError) -> Self {
        Self::Serve(source)
    }
}

impl From<ComponentError> for Error {
    fn from(source: ComponentError) -> Self {
        Self::Component(source)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn serve_and_preview_failures_have_their_own_codes() {
        let run = Error::from(ServeError::Run { source: std::io::Error::other("no signal") });
        let preview = Error::from(ServeError::Serve { source: std::io::Error::other("no socket") });
        assert_eq!((run.kind(), preview.kind()), ("serve.run.failed", "preview.serve.failed"));
    }
}
