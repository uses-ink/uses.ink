//! What the recovery store fails with.

use std::fmt;

use crate::unreadable::Unreadable;

/// A recovery store call that failed. `E` is the database adapter's own error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// The database failed.
    Db(db::Error<E>),
    /// A stored row is not what this version writes.
    Unreadable(Unreadable),
    /// A statement could not be built.
    Statement(sea_query::error::Error),
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(_) => out.write_str("the database failed"),
            Self::Unreadable(_) => out.write_str("a stored row is not what this version writes"),
            Self::Statement(_) => out.write_str("a statement could not be built"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Db(source) => Some(source),
            Self::Unreadable(source) => Some(source),
            Self::Statement(source) => Some(source),
        }
    }
}

impl<E> From<db::Error<E>> for Error<E> {
    fn from(source: db::Error<E>) -> Self {
        Self::Db(source)
    }
}

impl<E> From<Unreadable> for Error<E> {
    fn from(source: Unreadable) -> Self {
        Self::Unreadable(source)
    }
}

impl<E> From<db::ColumnError> for Error<E> {
    fn from(source: db::ColumnError) -> Self {
        Self::Unreadable(source.into())
    }
}

impl<E> From<sea_query::error::Error> for Error<E> {
    fn from(source: sea_query::error::Error) -> Self {
        Self::Statement(source)
    }
}
