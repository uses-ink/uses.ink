//! What the SQLite adapter fails with.

use std::fmt;
use std::path::PathBuf;
use std::str::Utf8Error;

/// What the adapter was doing when SQLite refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stage {
    /// Running a statement of this shape.
    Statement(db::Shape),
    /// Opening the transaction of a batch.
    Begin,
    /// Committing the transaction of a batch.
    Commit,
}

impl fmt::Display for Stage {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Statement(shape) => write!(out, "{shape}"),
            Self::Begin => out.write_str("the start of a batch"),
            Self::Commit => out.write_str("the end of a batch"),
        }
    }
}

/// A failure of the SQLite adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The database could not be opened or set up.
    Open {
        /// The file, or none for memory.
        path: Option<PathBuf>,
        /// What SQLite said.
        source: rusqlite::Error,
    },
    /// A statement, or the transaction around a batch, failed.
    Run {
        /// What was being run.
        stage: Stage,
        /// What SQLite said.
        source: rusqlite::Error,
    },
    /// A statement could not be rendered.
    Render {
        /// Which statement of the batch, counting from zero; zero for a single one.
        index: usize,
        /// Why.
        source: db::Unrenderable,
    },
    /// A text column holds bytes that are not UTF-8.
    Text {
        /// The column.
        column: String,
        /// Where the text stops being UTF-8.
        source: Utf8Error,
    },
    /// A thread panicked while it held the connection.
    Poisoned,
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { path: Some(path), .. } => write!(out, "the database {} could not be opened", path.display()),
            Self::Open { path: None, .. } => out.write_str("the in-memory database could not be opened"),
            Self::Run { stage, .. } => write!(out, "SQLite refused {stage}"),
            Self::Render { index, .. } => write!(out, "statement {index} could not be rendered"),
            Self::Text { column, .. } => write!(out, "column `{column}` holds text that is not UTF-8"),
            Self::Poisoned => out.write_str("a thread panicked while it held the connection"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. } | Self::Run { source, .. } => Some(source),
            Self::Render { source, .. } => Some(source),
            Self::Text { source, .. } => Some(source),
            Self::Poisoned => None,
        }
    }
}
