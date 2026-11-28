//! The failures of a call that ran on the ports.

use std::fmt;

use blob::Blob;
use db::Db;
use queue::Queue;
use secret::Secret;

use super::{Refusal, Unreadable};
use crate::Ports;

/// A control plane call that failed. `P` is the ports it ran on.
#[non_exhaustive]
pub enum Error<P: Ports> {
    /// The request is refused.
    Refused(Refusal),
    /// The database failed.
    Db(db::Error<<P::Db as Db>::Error>),
    /// The blob store failed.
    Blob(blob::Error<<P::Blob as Blob>::Error>),
    /// The queue failed.
    Queue(queue::Error<<P::Queue as Queue>::Error>),
    /// The sealer failed.
    Secret(secret::Error<<P::Secret as Secret>::Error>),
    /// A stored row is not what this version writes.
    Unreadable(Unreadable),
    /// A build's log or a site event could not be written as JSON.
    Log(serde_json::Error),
    /// A statement could not be built: its columns and values do not match.
    Statement(sea_query::error::Error),
    /// A stored manifest is not in the canonical form.
    Manifest(crate::listed::Canonical),
    /// A site's address could not be made from the origin.
    Origin(model::UrlError),
    /// The brand list this build carries does not read.
    Brand(crate::BrandError),
    /// The keys store failed or refused: an organization's key.
    Keys(crate::keys::Error<<P::Db as Db>::Error>),
}

impl<P: Ports> fmt::Debug for Error<P> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) => out.debug_tuple("Refused").field(error).finish(),
            Self::Db(error) => out.debug_tuple("Db").field(error).finish(),
            Self::Blob(error) => out.debug_tuple("Blob").field(error).finish(),
            Self::Queue(error) => out.debug_tuple("Queue").field(error).finish(),
            Self::Secret(error) => out.debug_tuple("Secret").field(error).finish(),
            Self::Unreadable(error) => out.debug_tuple("Unreadable").field(error).finish(),
            Self::Log(error) => out.debug_tuple("Log").field(error).finish(),
            Self::Statement(error) => out.debug_tuple("Statement").field(error).finish(),
            Self::Manifest(error) => out.debug_tuple("Manifest").field(error).finish(),
            Self::Origin(error) => out.debug_tuple("Origin").field(error).finish(),
            Self::Brand(error) => out.debug_tuple("Brand").field(error).finish(),
            Self::Keys(error) => out.debug_tuple("Keys").field(error).finish(),
        }
    }
}

impl<P: Ports> fmt::Display for Error<P> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => refusal.fmt(out),
            Self::Db(_) => out.write_str("the database failed"),
            Self::Blob(_) => out.write_str("the blob store failed"),
            Self::Queue(_) => out.write_str("the queue failed"),
            Self::Secret(_) => out.write_str("sealing failed"),
            Self::Unreadable(_) => out.write_str("a stored row is not what this version writes"),
            Self::Log(_) => out.write_str("a build's log could not be written"),
            Self::Statement(_) => out.write_str("a statement could not be built"),
            Self::Manifest(_) => out.write_str("a stored manifest is not in the canonical form"),
            Self::Origin(_) => out.write_str("a site's address could not be made from the origin"),
            Self::Brand(_) => out.write_str("the brand list of this build does not read"),
            Self::Keys(_) => out.write_str("the keys store failed or refused"),
        }
    }
}

impl<P: Ports> std::error::Error for Error<P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(source) => Some(source),
            Self::Db(source) => Some(source),
            Self::Blob(source) => Some(source),
            Self::Queue(source) => Some(source),
            Self::Secret(source) => Some(source),
            Self::Unreadable(source) => Some(source),
            Self::Log(source) => Some(source),
            Self::Statement(source) => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Origin(source) => Some(source),
            Self::Brand(source) => Some(source),
            Self::Keys(source) => Some(source),
        }
    }
}

impl<P: Ports> From<crate::keys::Error<<P::Db as Db>::Error>> for Error<P> {
    fn from(error: crate::keys::Error<<P::Db as Db>::Error>) -> Self {
        Self::Keys(error)
    }
}

impl<P: Ports> From<sea_query::error::Error> for Error<P> {
    fn from(source: sea_query::error::Error) -> Self {
        Self::Statement(source)
    }
}

impl<P: Ports> From<crate::event::Unwritten> for Error<P> {
    fn from(error: crate::event::Unwritten) -> Self {
        match error {
            crate::event::Unwritten::Json(source) => Self::Log(source),
            crate::event::Unwritten::Statement(source) => Self::Statement(source),
        }
    }
}

impl<P: Ports> From<crate::BrandError> for Error<P> {
    fn from(error: crate::BrandError) -> Self {
        Self::Brand(error)
    }
}

impl<P: Ports> From<Refusal> for Error<P> {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl<P: Ports> From<Unreadable> for Error<P> {
    fn from(error: Unreadable) -> Self {
        Self::Unreadable(error)
    }
}

impl<P: Ports> From<crate::unreadable::Unreadable> for Error<P> {
    fn from(error: crate::unreadable::Unreadable) -> Self {
        Self::Unreadable(error.into())
    }
}

impl<P: Ports> From<db::ColumnError> for Error<P> {
    fn from(error: db::ColumnError) -> Self {
        Self::Unreadable(error.into())
    }
}

impl<P: Ports> From<db::Error<<P::Db as Db>::Error>> for Error<P> {
    fn from(error: db::Error<<P::Db as Db>::Error>) -> Self {
        Self::Db(error)
    }
}

impl<P: Ports> From<blob::Error<<P::Blob as Blob>::Error>> for Error<P> {
    fn from(error: blob::Error<<P::Blob as Blob>::Error>) -> Self {
        Self::Blob(error)
    }
}

impl<P: Ports> From<queue::Error<<P::Queue as Queue>::Error>> for Error<P> {
    fn from(error: queue::Error<<P::Queue as Queue>::Error>) -> Self {
        Self::Queue(error)
    }
}

impl<P: Ports> From<secret::Error<<P::Secret as Secret>::Error>> for Error<P> {
    fn from(error: secret::Error<<P::Secret as Secret>::Error>) -> Self {
        Self::Secret(error)
    }
}

impl<P: Ports> Error<P> {
    /// The refusal this is, if the request was refused rather than failed.
    pub const fn refusal(&self) -> Option<&Refusal> {
        match self {
            Self::Refused(refusal) => Some(refusal),
            _ => None,
        }
    }
}

/// A call that ran on the control plane's ports and on one more: the builder, or the sink an
/// export writes to. `X` is that one's own error.
#[non_exhaustive]
pub enum Failed<P: Ports, X> {
    /// The control plane failed or refused.
    Control(Error<P>),
    /// The builder or the sink failed.
    Other(X),
}

impl<P: Ports, X: fmt::Debug> fmt::Debug for Failed<P, X> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(error) => out.debug_tuple("Control").field(error).finish(),
            Self::Other(error) => out.debug_tuple("Other").field(error).finish(),
        }
    }
}

impl<P: Ports, X> fmt::Display for Failed<P, X> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(_) => out.write_str("the control plane failed"),
            Self::Other(_) => out.write_str("the builder or the sink failed"),
        }
    }
}

impl<P: Ports + 'static, X: std::error::Error + 'static> std::error::Error for Failed<P, X> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Control(source) => Some(source),
            Self::Other(source) => Some(source),
        }
    }
}

impl<P: Ports, X> From<Error<P>> for Failed<P, X> {
    fn from(error: Error<P>) -> Self {
        Self::Control(error)
    }
}
