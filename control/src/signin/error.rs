//! What signing in fails with.

use std::fmt;

use mail::Address;

use crate::unreadable::Unreadable;

/// A request refused for a reason about the request. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// The token is not one that can be used: it was never made, it was used, it ended, or it
    /// is another kind of token. The answer does not say which, so it cannot be probed.
    Unusable,
    /// The sign-up policy no longer lets this address make an account.
    Closed,
    /// The address already belongs to an account.
    Taken {
        /// The address.
        address: Address,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unusable => out.write_str("that link cannot be used"),
            Self::Closed => out.write_str("this instance does not make accounts for new addresses"),
            Self::Taken { address } => write!(out, "{address} already belongs to an account"),
        }
    }
}

impl std::error::Error for Refused {}

/// A sign-in call that failed. `E` is the database adapter's own error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// The request is refused.
    Refused(Refused),
    /// The database failed.
    Db(db::Error<E>),
    /// A stored row is not what this version writes.
    Unreadable(Unreadable),
    /// A statement could not be built.
    Statement(sea_query::error::Error),
    /// A time could not be worked out.
    Time(jiff::Error),
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => refusal.fmt(out),
            Self::Db(_) => out.write_str("the database failed"),
            Self::Unreadable(_) => out.write_str("a stored row is not what this version writes"),
            Self::Statement(_) => out.write_str("a statement could not be built"),
            Self::Time(_) => out.write_str("a time could not be worked out"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(source) => Some(source),
            Self::Db(source) => Some(source),
            Self::Unreadable(source) => Some(source),
            Self::Statement(source) => Some(source),
            Self::Time(source) => Some(source),
        }
    }
}

impl<E> From<Refused> for Error<E> {
    fn from(source: Refused) -> Self {
        Self::Refused(source)
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

impl<E> From<jiff::Error> for Error<E> {
    fn from(source: jiff::Error) -> Self {
        Self::Time(source)
    }
}

impl<E> Error<E> {
    /// The refusal this is, if the request was refused rather than failed.
    pub const fn refusal(&self) -> Option<&Refused> {
        match self {
            Self::Refused(refusal) => Some(refusal),
            _ => None,
        }
    }
}
