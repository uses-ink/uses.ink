//! What the edge decided about one request, beside the response that carries it out.

use std::error::Error;
use std::fmt;

use model::{Hash, Route};

use crate::http::encoding::Encoding;
use crate::http::fields::FieldError;
use crate::http::range::{Ignored, Span};
use crate::http::refusal::Refusal;
use crate::http::response::Response;
use crate::serve::hostname::Hostname;
use crate::serve::port::{Blob, BlobError};

/// What went wrong on our side: a store failed, or the deploy holds what it should not.
#[derive(Debug)]
#[non_exhaustive]
pub enum Fault<S, B> {
    /// The site lookup failed.
    Sites(S),
    /// A blob could not be opened.
    Blob(BlobError<B>),
    /// A file is larger than the operator allows serving.
    Large {
        /// The blob.
        hash: Hash,
        /// Its length.
        length: u64,
        /// The limit.
        limit: u64,
    },
    /// The edge wrote a header that is not one.
    Field(FieldError),
}

impl<S, B> fmt::Display for Fault<S, B> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sites(_) => write!(out, "the site lookup failed"),
            Self::Blob(_) => write!(out, "a blob could not be read"),
            Self::Large { hash, length, limit } => {
                write!(out, "the blob {hash} is {length} bytes, over the serving limit of {limit}")
            }
            Self::Field(_) => write!(out, "the edge wrote a bad header"),
        }
    }
}

impl<S: Error + 'static, B: Error + 'static> Error for Fault<S, B> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sites(source) => Some(source),
            Self::Blob(source) => Some(source),
            Self::Large { .. } => None,
            Self::Field(source) => Some(source),
        }
    }
}

/// How a file's bytes were chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ranging {
    /// All of it.
    Whole,
    /// One span, answered `206`.
    Span(Span),
    /// A `Range` the edge did not follow, for this reason.
    Ignored(Ignored),
}

/// What happened to a request. Every answer has one, so a log line or a test can say more than a
/// status does.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome<F> {
    /// A route of the deploy, served.
    Served {
        /// The route.
        route: Route,
        /// How its bytes were chosen.
        ranging: Ranging,
        /// The coding of the variant served, when it is one.
        encoding: Option<Encoding>,
    },
    /// The client already holds the file: `304`.
    Unchanged {
        /// The route.
        route: Route,
    },
    /// The route moved: `301`.
    Moved {
        /// The old route.
        from: Route,
        /// The new one.
        to: Route,
    },
    /// The deploy has no such route: `404`, with the site's page when it has one.
    Missing {
        /// The route, when the path names one.
        route: Option<Route>,
    },
    /// The load balancer's probe.
    Probed,
    /// The request was refused.
    Refused(Refusal),
    /// The edge or its stores failed: `500`.
    Failed(F),
}

/// A response, why it is that one, and who it was for.
#[derive(Debug)]
pub struct Answer<R: Blob, F> {
    /// The response to send.
    pub response: Response<R>,
    /// What happened.
    pub outcome: Outcome<F>,
    /// The host the request named, when it got far enough to name one.
    pub hostname: Option<Hostname>,
}
