//! The API's requests and replies, as values: no HTTP library is named here.

use std::fmt;
use std::net::IpAddr;
use std::time::Duration;

use crate::cookie::Cookie;
use crate::door::Enter;

/// An HTTP method the API has a route for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `DELETE`.
    Delete,
}

impl Method {
    /// Every method the API has a route for.
    pub const ALL: [Self; 4] = [Self::Get, Self::Post, Self::Put, Self::Delete];
}

impl fmt::Display for Method {
    /// The method as written on the wire: its name in capitals.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&format!("{self:?}").to_ascii_uppercase())
    }
}

/// A method that has no route anywhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownMethod {
    /// What was written.
    pub written: String,
}

impl fmt::Display for UnknownMethod {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:?} is no method the API answers", self.written)
    }
}

impl std::error::Error for UnknownMethod {}

impl std::str::FromStr for Method {
    type Err = UnknownMethod;

    fn from_str(text: &str) -> Result<Self, UnknownMethod> {
        Self::ALL
            .into_iter()
            .find(|method| method.to_string() == text)
            .ok_or_else(|| UnknownMethod { written: text.to_owned() })
    }
}

/// A request to the API: what the adapter read, with the body already bounded. It never prints
/// a credential: `Debug` shows the `Authorization` and `Cookie` headers as present or absent
/// and the token of a sign-in link in the path as `<redacted>`.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// The method.
    pub method: Method,
    /// The path, without its query, as received.
    pub path: String,
    /// The `Authorization` header's value, if there was one. Never printed.
    pub authorization: Option<Vec<u8>>,
    /// The `Cookie` header's value, if there was one. Never printed.
    pub cookie: Option<Vec<u8>>,
    /// The address the connection came from, as the adapter saw it. Behind a reverse proxy this
    /// is the proxy's, so a limit per source is then a limit for everyone behind it.
    pub source: Option<IpAddr>,
    /// The body, or that it went over the limit the adapter was given.
    pub body: Payload,
}

impl fmt::Debug for Request {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held = |header: &Option<Vec<u8>>| header.as_ref().map(|_| "<redacted>");
        out.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &Enter::redact(&self.path))
            .field("authorization", &held(&self.authorization))
            .field("cookie", &held(&self.cookie))
            .field("source", &self.source)
            .field("body", &self.body)
            .finish()
    }
}

/// A request's body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Payload {
    /// All of it.
    Whole(Vec<u8>),
    /// More than `limit` bytes were sent, and the adapter stopped reading.
    Over {
        /// The limit it read up to.
        limit: u64,
    },
}

/// The HTTP statuses the API answers with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Status {
    /// `200`.
    Ok,
    /// `201`: something was made.
    Created,
    /// `202`: the request was taken and its effect, if any, comes later.
    Accepted,
    /// `400`: the request is malformed.
    BadRequest,
    /// `401`: no valid token.
    Unauthorized,
    /// `404`: nothing is there, or it is not the caller's.
    NotFound,
    /// `405`.
    MethodNotAllowed,
    /// `408`: the body did not arrive in time.
    Timeout,
    /// `409`: the request is fine and the state says no.
    Conflict,
    /// `413`: over a limit.
    TooLarge,
    /// `429`: too many requests; `Retry-After` says when to try again.
    TooMany,
    /// `422`: well formed, and not something the plane can do with.
    Unprocessable,
    /// `500`.
    Internal,
}

impl Status {
    /// The status code.
    pub const fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Created => 201,
            Self::Accepted => 202,
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::Timeout => 408,
            Self::Conflict => 409,
            Self::TooLarge => 413,
            Self::Unprocessable => 422,
            Self::TooMany => 429,
            Self::Internal => 500,
        }
    }
}

/// What a body is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Media {
    /// No body.
    Empty,
    /// One JSON value.
    Json,
    /// A JSON value a line.
    Ndjson,
    /// A tar archive.
    Tar,
}

impl Media {
    /// The `Content-Type` of a body of this kind, if it has a body.
    pub const fn content_type(self) -> Option<&'static str> {
        match self {
            Self::Empty => None,
            Self::Json => Some("application/json"),
            Self::Ndjson => Some("application/x-ndjson"),
            Self::Tar => Some("application/x-tar"),
        }
    }
}

/// The API's answer. It can carry one `Set-Cookie`, whose value is a secret and does not print;
/// its body can hold one too (a new session), so `Debug` shows the body's length and no more.
#[derive(Clone, PartialEq, Eq)]
pub struct Reply {
    /// The status.
    pub status: Status,
    /// What the body is.
    pub media: Media,
    /// The body.
    pub body: Vec<u8>,
    /// The methods the route takes, for a `405`.
    pub allow: Vec<Method>,
    /// Whether the answer asks for a bearer token (`WWW-Authenticate`), for a `401`.
    pub challenge: bool,
    /// The cookie to set, for a sign-in or a sign-out.
    pub cookie: Option<Cookie>,
    /// How long to wait before asking again (`Retry-After`), for a `429`.
    pub retry: Option<Duration>,
}

impl fmt::Debug for Reply {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Reply")
            .field("status", &self.status)
            .field("media", &self.media)
            .field("body", &format_args!("{} bytes", self.body.len()))
            .field("allow", &self.allow)
            .field("challenge", &self.challenge)
            .field("cookie", &self.cookie)
            .field("retry", &self.retry)
            .finish()
    }
}

impl Reply {
    /// A reply with a body.
    pub const fn new(status: Status, media: Media, body: Vec<u8>) -> Self {
        Self { status, media, body, allow: Vec::new(), challenge: false, cookie: None, retry: None }
    }

    /// A reply with no body.
    pub const fn empty(status: Status) -> Self {
        Self::new(status, Media::Empty, Vec::new())
    }
}
