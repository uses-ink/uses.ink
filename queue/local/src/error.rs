//! What the local queue fails with.

use std::fmt;

/// A failure of the local queue. `E` is the database adapter's error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// The database failed.
    Db(db::Error<E>),
    /// A row of the queue table cannot be read.
    Row(db::ColumnError),
    /// A time that does not fit: a lease or delay too long to add to the clock.
    Time {
        /// What the clock arithmetic said.
        source: jiff::Error,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(_) => out.write_str("the queue table could not be used"),
            Self::Row(_) => out.write_str("a queue row cannot be read"),
            Self::Time { .. } => out.write_str("the lease or delay does not fit in the clock"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Db(source) => Some(source),
            Self::Row(source) => Some(source),
            Self::Time { source } => Some(source),
        }
    }
}

impl<E> From<db::Error<E>> for Error<E> {
    fn from(source: db::Error<E>) -> Self {
        Self::Db(source)
    }
}

impl<E> From<db::ColumnError> for Error<E> {
    fn from(source: db::ColumnError) -> Self {
        Self::Row(source)
    }
}
