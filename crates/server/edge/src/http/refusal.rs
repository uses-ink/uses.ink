//! Every way the edge says no, as data. Each variant knows its status; its text is its `Display`.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use crate::http::condition::ConditionError;
use crate::http::head::{BodyError, HeaderError};
use crate::http::response::Status;
use crate::serve::hostname::{Hostname, HostnameError};
use crate::serve::target::TargetError;

/// Which wait ran out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stage {
    /// The client sent its request head too slowly.
    Head,
    /// A site or blob lookup took too long.
    Open,
    /// The reader stopped taking the response.
    Idle,
    /// The whole response took too long.
    Send,
}

impl fmt::Display for Stage {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Head => "reading the request head",
            Self::Open => "looking up the site and its blob",
            Self::Idle => "waiting for the reader to take a chunk",
            Self::Send => "sending the response",
        })
    }
}

/// A request the edge will not serve, and why. Nothing here is a fault of the edge or the deploy.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// A method other than `GET` and `HEAD`.
    Method {
        /// The method, as sent.
        method: String,
    },
    /// The request target is hostile or over a limit.
    Target(TargetError),
    /// The `Host` header is missing, repeated or no host.
    Hostname(HostnameError),
    /// An absolute-form target names another host than the `Host` header.
    Authority {
        /// The `Host` header's host.
        header: Hostname,
        /// The target's host.
        target: Hostname,
    },
    /// The headers are over a limit or malformed.
    Headers(HeaderError),
    /// A body on a request that has none.
    Body(BodyError),
    /// A conditional header that does not parse.
    Condition(ConditionError),
    /// A `Range` that starts past the end of a file of this length.
    Unsatisfiable {
        /// The file's length.
        length: u64,
    },
    /// No site has this name.
    Unserved {
        /// The name.
        hostname: Hostname,
    },
    /// More requests in flight than the limit.
    Busy {
        /// The limit.
        limit: usize,
    },
    /// A wait ran out.
    Slow {
        /// Which.
        stage: Stage,
        /// Its budget.
        limit: Duration,
    },
}

impl Refusal {
    /// The status the refusal is answered with.
    pub const fn status(&self) -> Status {
        match self {
            Self::Method { .. } => Status::MethodNotAllowed,
            Self::Target(TargetError::TooLong { .. }) => Status::UriTooLong,
            Self::Headers(HeaderError::Count { .. } | HeaderError::Size { .. } | HeaderError::Total { .. }) => {
                Status::HeadersTooLarge
            }
            Self::Target(_)
            | Self::Hostname(_)
            | Self::Authority { .. }
            | Self::Headers(_)
            | Self::Body(_)
            | Self::Condition(_) => Status::BadRequest,
            Self::Unsatisfiable { .. } => Status::RangeNotSatisfiable,
            Self::Unserved { .. } => Status::NotFound,
            Self::Busy { .. } => Status::Unavailable,
            Self::Slow { stage: Stage::Head, .. } => Status::RequestTimeout,
            Self::Slow { .. } => Status::GatewayTimeout,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Method { method } => write!(out, "the method {method:?} is not served; use GET or HEAD"),
            Self::Target(_) => write!(out, "the request target is refused"),
            Self::Hostname(_) => write!(out, "the host of the request is refused"),
            Self::Authority { header, target } => write!(out, "the host header names {header} and the target {target}"),
            Self::Headers(_) => write!(out, "the request headers are refused"),
            Self::Body(_) => write!(out, "the request body is refused"),
            Self::Condition(_) => write!(out, "a conditional header is refused"),
            Self::Unsatisfiable { length } => write!(out, "the range starts past the end of a file of {length} bytes"),
            Self::Unserved { hostname } => write!(out, "no site is served at {hostname}"),
            Self::Busy { limit } => write!(out, "more than {limit} requests are in flight"),
            Self::Slow { stage, limit } => write!(out, "{stage} took more than {limit:?}"),
        }
    }
}

impl Error for Refusal {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Target(source) => Some(source),
            Self::Hostname(source) => Some(source),
            Self::Headers(source) => Some(source),
            Self::Body(source) => Some(source),
            Self::Condition(source) => Some(source),
            _ => None,
        }
    }
}

impl From<TargetError> for Refusal {
    fn from(source: TargetError) -> Self {
        Self::Target(source)
    }
}

impl From<HostnameError> for Refusal {
    fn from(source: HostnameError) -> Self {
        Self::Hostname(source)
    }
}

impl From<HeaderError> for Refusal {
    fn from(source: HeaderError) -> Self {
        Self::Headers(source)
    }
}

impl From<BodyError> for Refusal {
    fn from(source: BodyError) -> Self {
        Self::Body(source)
    }
}

impl From<ConditionError> for Refusal {
    fn from(source: ConditionError) -> Self {
        Self::Condition(source)
    }
}
