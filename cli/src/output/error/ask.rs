//! What a person gave a command to name a file or a decision, and the command could not use.

use std::fmt;

/// A path, a code or a pattern given on the command line that names nothing usable.
#[derive(Debug)]
#[non_exhaustive]
pub enum AskError {
    /// The path given to `ink explain` is no path of a site.
    Path {
        /// The path, as given.
        written: String,
        /// Why it is no path.
        source: model::PathError,
    },
    /// A decision code given on the command line is no code.
    Code {
        /// Why.
        source: model::UnknownDecision,
    },
    /// A code given with a path is decided on the site, or one given without is decided per
    /// file.
    Scope {
        /// The code.
        code: model::AnyCode,
        /// What it was asked about.
        subject: model::Subject,
    },
    /// `--why` was given neither a code nor a path and a code.
    Words {
        /// How many words it was given.
        given: usize,
    },
    /// The decision asked about is never made, as for a file that is ignored.
    Undecided {
        /// The code.
        code: model::AnyCode,
        /// What it was asked about.
        subject: model::Subject,
    },
    /// The path given to `ink explain` is no file of the folder.
    Unknown {
        /// The path.
        path: model::ContentPath,
    },
    /// A file `ink select` was asked about is not a page whose frontmatter selects it.
    Unselectable {
        /// The file.
        path: model::ContentPath,
        /// Why not.
        why: crate::command::select::Unselectable,
    },
    /// `--folder` is no glob pattern.
    Folder {
        /// The pattern, as given.
        written: String,
        /// Why it is none.
        source: config::PatternError,
    },
}

impl AskError {
    /// What kind of error it is, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Path { .. } => "explain.path.invalid",
            Self::Unknown { .. } => "explain.path.unknown",
            Self::Code { .. } => "decision.code.unknown",
            Self::Scope { .. } => "decision.code.scope",
            Self::Words { .. } => "decision.words.invalid",
            Self::Undecided { .. } => "decision.subject.undecided",
            Self::Unselectable { .. } => "select.file.unselectable",
            Self::Folder { .. } => "select.folder.invalid",
        }
    }
}

impl fmt::Display for AskError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path { written, .. } => write!(out, "`{written}` is not a path in the folder"),
            Self::Unknown { path } => write!(out, "{path} is not a file of the folder"),
            Self::Code { .. } => write!(out, "no decision has that code"),
            Self::Scope { code, subject: model::Subject::File(path) } => {
                write!(out, "`{code}` is decided once for the site, not on {path}: give the code alone")
            }
            Self::Scope { code, .. } => write!(out, "`{code}` is decided per file: give the file's path before it"),
            Self::Words { given } => {
                write!(out, "a decision is named by a code, or a path and a code, not {given} words")
            }
            Self::Undecided { code, subject } => {
                write!(out, "`{code}` is not decided on {subject}; `ink explain` lists what is")
            }
            Self::Unselectable { path, why } => write!(out, "{path} cannot be selected: {why}"),
            Self::Folder { written, .. } => write!(out, "`{written}` is not a glob pattern"),
        }
    }
}

impl std::error::Error for AskError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path { source, .. } => Some(source),
            Self::Code { source } => Some(source),
            Self::Folder { source, .. } => Some(source),
            Self::Unknown { .. }
            | Self::Unselectable { .. }
            | Self::Scope { .. }
            | Self::Words { .. }
            | Self::Undecided { .. } => None,
        }
    }
}
