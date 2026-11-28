//! What the passkey store fails with.

use std::fmt;

use crate::unreadable::Unreadable;

/// A request the store refuses for a reason about the request. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// The credential is already registered.
    Taken,
    /// The account has no passkey with that id.
    Missing,
    /// The credential's counter changed between reading it and writing it: another sign-in
    /// with the same passkey won. Nothing was written.
    Raced,
    /// The server is set up for another relying party id than the one passkeys were made for.
    Moved {
        /// The id the first passkey was made under.
        was: String,
        /// The id now configured.
        now: String,
        /// How many passkeys would stop working.
        passkeys: u64,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Taken => out.write_str("that passkey is already registered"),
            Self::Missing => out.write_str("the account has no passkey with that id"),
            Self::Raced => out.write_str("the passkey was used at the same moment by another sign-in"),
            Self::Moved { was, now, passkeys } => write!(
                out,
                "passkeys were made for `{was}` and the relying party id is now `{now}`; {passkeys} would stop working"
            ),
        }
    }
}

impl std::error::Error for Refused {}

/// A passkey store call that failed. `E` is the database adapter's own error.
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
