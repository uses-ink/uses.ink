//! What a key call fails with.

use std::fmt;

use super::key::Kind;
use crate::Unreadable;
use crate::signin::Ttl;

/// A request refused for a reason about the request. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// No enrolment is waiting under that code or id. A code that never existed, one that
    /// expired and one that was used are the same answer, so codes cannot be probed.
    NoEnrolment,
    /// The code names an enrolment whose key is not the one the person compared.
    Fingerprint,
    /// No key of the account has that fingerprint.
    NoKey,
    /// A deploy key needs a site of the approving account.
    Site,
    /// The rights asked for are more than the approver has.
    Rights,
    /// A lifetime above the operator's ceiling.
    Lifetime {
        /// What was asked.
        asked: Ttl,
        /// The ceiling.
        ceiling: Ttl,
    },
    /// The code is already in use by another pending enrolment.
    CodeTaken,
    /// The key is already enrolled or already pending.
    KeyTaken,
    /// Only a kind of key has this.
    Kind {
        /// The kind.
        kind: Kind,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEnrolment => out.write_str("that code is unknown or has expired"),
            Self::Fingerprint => out.write_str("that is not the key fingerprint of the request"),
            Self::NoKey => out.write_str("no key of this account has that fingerprint"),
            Self::Site => out.write_str("a deploy key is for one site of the approving account"),
            Self::Rights => out.write_str("a key cannot have more rights than the person who approves it"),
            Self::Lifetime { asked, ceiling } => write!(out, "a key of {asked} is over the ceiling of {ceiling}"),
            Self::CodeTaken => out.write_str("another enrolment holds that code"),
            Self::KeyTaken => out.write_str("that key is already enrolled or waiting"),
            Self::Kind { kind } => write!(out, "this is not for a {kind} key"),
        }
    }
}

impl std::error::Error for Refused {}

/// A key call that failed. `E` is the database adapter's own error.
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
    /// An event could not be written as JSON.
    Json(serde_json::Error),
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => refusal.fmt(out),
            Self::Db(_) => out.write_str("the database failed"),
            Self::Unreadable(_) => out.write_str("a stored row is not what this version writes"),
            Self::Statement(_) => out.write_str("a statement could not be built"),
            Self::Time(_) => out.write_str("a time could not be worked out"),
            Self::Json(_) => out.write_str("an event could not be written"),
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
            Self::Json(source) => Some(source),
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

impl<E> From<serde_json::Error> for Error<E> {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(source)
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
