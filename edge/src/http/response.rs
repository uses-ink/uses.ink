//! What the edge answers: a status, header fields and a body.

use std::fmt;

use model::{HeaderName, HeaderValue};

use crate::http::range::Span;
use crate::serve::port::Blob;

/// The HTTP statuses the edge answers with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Status {
    /// `200`.
    Ok,
    /// `206`: a span of a file.
    Partial,
    /// `301`: a route that moved.
    Moved,
    /// `304`: the client holds this file.
    NotModified,
    /// `400`: a request that is malformed or hostile.
    BadRequest,
    /// `404`.
    NotFound,
    /// `405`.
    MethodNotAllowed,
    /// `408`: the client was too slow sending its request.
    RequestTimeout,
    /// `414`.
    UriTooLong,
    /// `416`.
    RangeNotSatisfiable,
    /// `431`.
    HeadersTooLarge,
    /// `500`: the edge could not read what the deploy says it holds.
    Internal,
    /// `503`: too many requests in flight.
    Unavailable,
    /// `504`: a lookup took longer than its budget.
    GatewayTimeout,
}

impl Status {
    /// The status code.
    pub const fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Partial => 206,
            Self::Moved => 301,
            Self::NotModified => 304,
            Self::BadRequest => 400,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::RequestTimeout => 408,
            Self::UriTooLong => 414,
            Self::RangeNotSatisfiable => 416,
            Self::HeadersTooLarge => 431,
            Self::Internal => 500,
            Self::Unavailable => 503,
            Self::GatewayTimeout => 504,
        }
    }
}

impl From<model::Status> for Status {
    fn from(status: model::Status) -> Self {
        match status {
            model::Status::NotFound => Self::NotFound,
            _ => Self::Ok,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.code())
    }
}

/// A response header's name: one a deploy's manifest may set, or one the edge sets itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Name {
    /// A header from a manifest entry.
    Entry(HeaderName),
    /// `accept-ranges`.
    AcceptRanges,
    /// `allow`.
    Allow,
    /// `content-encoding`.
    ContentEncoding,
    /// `content-length`.
    ContentLength,
    /// `content-range`.
    ContentRange,
    /// `content-type`.
    ContentType,
    /// `etag`.
    ETag,
    /// `location`.
    Location,
    /// `retry-after`.
    RetryAfter,
    /// `vary`.
    Vary,
}

impl Name {
    /// Lower case, as HTTP/2 writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Entry(name) => name.name(),
            Self::AcceptRanges => "accept-ranges",
            Self::Allow => "allow",
            Self::ContentEncoding => "content-encoding",
            Self::ContentLength => "content-length",
            Self::ContentRange => "content-range",
            Self::ContentType => "content-type",
            Self::ETag => "etag",
            Self::Location => "location",
            Self::RetryAfter => "retry-after",
            Self::Vary => "vary",
        }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// One response header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// Which header.
    pub name: Name,
    /// Its value.
    pub value: HeaderValue,
}

/// What follows the headers.
#[derive(Debug)]
#[non_exhaustive]
pub enum Body<R> {
    /// Nothing: a `HEAD`, a `304`, a redirect.
    Empty,
    /// A line of text the edge wrote: a refusal's reason.
    Text(String),
    /// The span of an opened blob. The adapter streams it.
    Blob {
        /// The blob.
        blob: R,
        /// Which of its bytes.
        span: Span,
    },
}

/// A status, headers and a body. `content-length` is always among the fields, so a `HEAD` says
/// what a `GET` would send.
#[derive(Debug)]
pub struct Response<R: Blob> {
    /// The status.
    pub status: Status,
    /// The headers, in the order to send them.
    pub fields: Vec<Field>,
    /// The body.
    pub body: Body<R>,
}

impl<R: Blob> Response<R> {
    /// The value of the first field called `name`.
    pub fn field(&self, name: Name) -> Option<&HeaderValue> {
        self.fields.iter().find(|field| field.name == name).map(|field| &field.value)
    }
}
