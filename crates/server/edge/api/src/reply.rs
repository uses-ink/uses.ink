//! The API's requests and replies, as values: no HTTP library is named here.

use std::fmt;
use std::net::IpAddr;
use std::time::Duration;

use crate::auth::cookie::Cookie;
use crate::auth::signin::Enter;

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
    /// The query, without its `?`, as received.
    pub query: Option<String>,
    /// The `Authorization` header's value, if there was one. Never printed.
    pub authorization: Option<Vec<u8>>,
    /// The `Cookie` header's value, if there was one. Never printed.
    pub cookie: Option<Vec<u8>>,
    /// The `Origin` header's value, if there was one. Read only by the cookie routes that change
    /// state (`docs/auth.md` section 8, CSRF).
    pub origin: Option<String>,
    /// The `Sec-Fetch-Site` header's value, if there was one: the browser's own word on whether
    /// the request came from the same origin.
    pub site: Option<String>,
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
            .field("query", &self.query)
            .field("authorization", &held(&self.authorization))
            .field("cookie", &held(&self.cookie))
            .field("origin", &self.origin)
            .field("site", &self.site)
            .field("source", &self.source)
            .field("body", &self.body)
            .finish()
    }
}

impl Request {
    /// The request target as it is signed: the path and, when there is one, `?` and the query.
    /// It is refused unless it is canonical.
    pub fn target(&self) -> Result<sig::Target, sig::NotCanonical> {
        match &self.query {
            Some(query) => sig::Target::parse(&format!("{}?{query}", self.path)),
            None => sig::Target::parse(&self.path),
        }
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
    /// `303`: the change was made; the `Location` header names the page to read next.
    SeeOther,
    /// `400`: the request is malformed.
    BadRequest,
    /// `401`: no valid credential.
    Unauthorized,
    /// `403`: the credential is good and may not do this.
    Forbidden,
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
    /// `503`: not now; `Retry-After` says when.
    Unavailable,
}

impl Status {
    /// The status code.
    pub const fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Created => 201,
            Self::Accepted => 202,
            Self::SeeOther => 303,
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::Timeout => 408,
            Self::Conflict => 409,
            Self::TooLarge => 413,
            Self::Unprocessable => 422,
            Self::TooMany => 429,
            Self::Internal => 500,
            Self::Unavailable => 503,
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
    /// A page of the dashboard, which the answerer's own headers fence in.
    Html,
    /// The dashboard's one script, as static text.
    Script,
}

impl Media {
    /// The `Content-Type` of a body of this kind, if it has a body.
    pub const fn content_type(self) -> Option<&'static str> {
        match self {
            Self::Empty => None,
            Self::Json => Some("application/json"),
            Self::Ndjson => Some("application/x-ndjson"),
            Self::Tar => Some("application/x-tar"),
            Self::Html => Some("text/html; charset=utf-8"),
            Self::Script => Some("text/javascript; charset=utf-8"),
        }
    }
}

/// A header an answerer sets itself, such as a page's `Content-Security-Policy` or a redirect's
/// `Location`. The name is lower case, as HTTP/2 writes it, and is one of a fixed set the answerer
/// knows by heart, so it is a `&'static str`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The name, in lower case.
    pub name: &'static str,
    /// The value.
    pub value: String,
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
    /// Whether the answer names the scheme it takes (`WWW-Authenticate: Ink-Sig-1`).
    pub challenge: bool,
    /// The cookie to set, for a sign-in or a sign-out.
    pub cookie: Option<Cookie>,
    /// The cookie of a passkey ceremony, to set when one starts and to clear when it ends.
    pub ceremony: Option<Cookie>,
    /// How long to wait before asking again (`Retry-After`), for a `429` or a `503`.
    pub retry: Option<Duration>,
    /// Headers of the answerer's own, set after the adapter's defaults and replacing any of the
    /// same name: the dashboard's page policy, a redirect's `Location`, a download's name.
    pub headers: Vec<Header>,
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
            .field("ceremony", &self.ceremony)
            .field("retry", &self.retry)
            .field("headers", &self.headers)
            .finish()
    }
}

impl Reply {
    /// A reply with a body.
    pub const fn new(status: Status, media: Media, body: Vec<u8>) -> Self {
        Self {
            status,
            media,
            body,
            allow: Vec::new(),
            challenge: false,
            cookie: None,
            ceremony: None,
            retry: None,
            headers: Vec::new(),
        }
    }

    /// A reply with no body.
    pub const fn empty(status: Status) -> Self {
        Self::new(status, Media::Empty, Vec::new())
    }
}

impl logging::Kind for Method {
    fn kind(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Put => "put",
            Self::Delete => "delete",
        }
    }
}
