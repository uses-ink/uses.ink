//! What running a statement can fail with.

use std::fmt;

use crate::TooMany;

/// The constraint a write broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Constraint {
    /// A `UNIQUE` or primary key: the row already exists.
    Unique,
    /// A foreign key: the row points at one that does not exist, or is pointed at.
    Reference,
    /// A `NOT NULL` column was given null.
    Null,
    /// A `CHECK` was false.
    Check,
}

impl fmt::Display for Constraint {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Unique => "a unique key is already taken",
            Self::Reference => "a foreign key does not hold",
            Self::Null => "a column that cannot be null was null",
            Self::Check => "a check failed",
        })
    }
}

/// A statement or batch that failed. `E` is the adapter's own error, kept as the source.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// A statement bound more parameters than D1 accepts.
    Parameters {
        /// Which statement of the batch, counting from zero; zero for a single one.
        index: usize,
        /// How many it bound and how many are allowed.
        found: TooMany,
    },
    /// A write broke a constraint. In a batch nothing was written.
    Constraint {
        /// Which constraint.
        constraint: Constraint,
        /// The adapter's error.
        source: E,
    },
    /// The store failed for another reason.
    Backend {
        /// The adapter's error.
        source: E,
    },
}

impl<E> Error<E> {
    /// The constraint this broke, if it was a constraint.
    pub const fn constraint(&self) -> Option<Constraint> {
        match self {
            Self::Constraint { constraint, .. } => Some(*constraint),
            Self::Parameters { .. } | Self::Backend { .. } => None,
        }
    }
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameters { index, found } => write!(out, "statement {index}: {found}"),
            Self::Constraint { constraint, .. } => write!(out, "the write was refused: {constraint}"),
            Self::Backend { .. } => out.write_str("the database failed"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parameters { found, .. } => Some(found),
            Self::Constraint { source, .. } | Self::Backend { source } => Some(source),
        }
    }
}
