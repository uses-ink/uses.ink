//! What the D1 adapter fails with.

use std::fmt;

use db::Constraint;

/// What the adapter was doing when D1 refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stage {
    /// Running a statement of this shape.
    Statement(db::Shape),
    /// Running a batch.
    Batch,
}

impl fmt::Display for Stage {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Statement(shape) => write!(out, "{shape}"),
            Self::Batch => out.write_str("a batch"),
        }
    }
}

/// What D1 or the JavaScript boundary said, with D1's own cause text kept apart from the rest.
#[derive(Debug)]
pub struct Remote {
    error: worker::Error,
    cause: Option<String>,
}

impl Remote {
    /// The code SQLite gave a refused constraint, as D1's error text carries it.
    const CODES: [(&'static str, Constraint); 5] = [
        ("SQLITE_CONSTRAINT_UNIQUE", Constraint::Unique),
        ("SQLITE_CONSTRAINT_PRIMARYKEY", Constraint::Unique),
        ("SQLITE_CONSTRAINT_FOREIGNKEY", Constraint::Reference),
        ("SQLITE_CONSTRAINT_NOTNULL", Constraint::Null),
        ("SQLITE_CONSTRAINT_CHECK", Constraint::Check),
    ];

    /// The constraint D1 says was broken, if its text names one.
    pub fn constraint(&self) -> Option<Constraint> {
        let cause = self.cause.as_deref()?;
        Self::CODES.iter().find(|(code, _)| cause.contains(code)).map(|(_, constraint)| *constraint)
    }
}

impl From<worker::Error> for Remote {
    fn from(error: worker::Error) -> Self {
        let cause = match &error {
            worker::Error::D1(d1) => Some(d1.cause()),
            _ => None,
        };
        Self { error, cause }
    }
}

impl fmt::Display for Remote {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(cause) => write!(out, "D1 said: {cause}"),
            None => write!(out, "{}", self.error),
        }
    }
}

impl std::error::Error for Remote {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// A failure of the D1 adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// D1 refused a statement or a batch.
    Run {
        /// What was being run.
        stage: Stage,
        /// What D1 said.
        source: Remote,
    },
    /// A statement could not be rendered or its values bound.
    Render {
        /// Which statement of the batch, counting from zero; zero for a single one.
        index: usize,
        /// Why.
        source: db::Unrenderable,
    },
    /// D1 answered with a value the port cannot read back.
    Read {
        /// The column.
        column: String,
        /// What was wrong with it.
        source: Unread,
    },
    /// D1 answered without the count of changed rows, or with another number of results than
    /// statements.
    Meta {
        /// What was being run.
        stage: Stage,
    },
}

/// A value D1 handed over that cannot be read exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Unread {
    /// A number with no fraction beyond 2^53 - 1: an integer D1 rounded, or a real that no
    /// integer can be told from.
    Rounded {
        /// The number as JavaScript holds it.
        found: f64,
    },
    /// A number that is not a number: infinity or not-a-number.
    Finite,
}

impl fmt::Display for Unread {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rounded { found } => write!(out, "the number {found} is beyond 2^53 - 1, where D1 rounds integers"),
            Self::Finite => out.write_str("the number is not finite"),
        }
    }
}

impl std::error::Error for Unread {}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run { stage, .. } => write!(out, "D1 refused {stage}"),
            Self::Render { index, .. } => write!(out, "statement {index} could not be rendered"),
            Self::Read { column, .. } => write!(out, "column `{column}` holds a value that cannot be read exactly"),
            Self::Meta { stage } => write!(out, "D1's answer to {stage} did not say what it changed"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Run { source, .. } => Some(source),
            Self::Render { source, .. } => Some(source),
            Self::Read { source, .. } => Some(source),
            Self::Meta { .. } => None,
        }
    }
}

impl From<Error> for db::Error<Error> {
    fn from(source: Error) -> Self {
        match source {
            Error::Render { index, source } => Self::Unrenderable { index, source },
            Error::Run { stage, source } => match source.constraint() {
                Some(constraint) => Self::Constraint { constraint, source: Error::Run { stage, source } },
                None => Self::Backend { source: Error::Run { stage, source } },
            },
            other => Self::Backend { source: other },
        }
    }
}
