//! The port that talks to a forge's API, and its stand-in.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use url::Url;
use zeroize::Zeroize;

use crate::{Fail, Limits, Secret};

/// What a request does. A forge call here reads or creates; none deletes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Reads.
    Get,
    /// Creates.
    Post,
}

/// One call to a forge's API. It prints with its credential redacted.
#[derive(Debug)]
#[non_exhaustive]
pub struct Request {
    /// What it does.
    pub method: Method,
    /// Where.
    pub url: Url,
    /// The credential, sent as `Authorization: Bearer`.
    pub bearer: Option<Secret>,
    /// Headers beyond the credential and the content type.
    pub headers: Vec<(&'static str, String)>,
    /// The JSON body, or nothing.
    pub body: Vec<u8>,
    /// How long the transport waits for the whole call.
    pub timeout: Duration,
    /// How many bytes of the reply the transport reads.
    pub limit: usize,
}

impl Request {
    /// A call of `method` to `url`, with the limits of [`Limits::DEFAULT`] until the client
    /// stamps its own.
    pub fn new(method: Method, url: Url) -> Self {
        Self {
            method,
            url,
            bearer: None,
            headers: Vec::new(),
            body: Vec::new(),
            timeout: Limits::DEFAULT.timeout,
            limit: Limits::DEFAULT.reply,
        }
    }

    /// The same call with `secret` as its credential. The credential moves into the request, and
    /// goes out of scope with it.
    #[must_use]
    pub fn bearer(mut self, secret: Secret) -> Self {
        self.bearer = Some(secret);
        self
    }

    /// The same call with one header more.
    #[must_use]
    pub fn header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.push((name, value.to_owned()));
        self
    }

    /// The same call with `body` as its JSON.
    pub fn json(mut self, body: &impl Serialize) -> Result<Self, serde_json::Error> {
        self.body = serde_json::to_vec(body)?;
        Ok(self)
    }
}

/// What a forge answered. It prints its status and size, never its body: a reply can carry a
/// token.
#[derive(Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The body.
    pub body: Vec<u8>,
}

impl Response {
    /// An answer of `status` with `body`.
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self { status, body: body.into() }
    }

    /// Whether the status is 2xx.
    pub const fn success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// The body as JSON of type `T`.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    /// The body as text, when it is UTF-8.
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.body).ok()
    }
}

impl Drop for Response {
    fn drop(&mut self) {
        self.body.zeroize();
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Response {{ status: {}, body: <{} bytes> }}", self.status, self.body.len())
    }
}

/// Sends one request and returns the answer. Futures are not required to be `Send`, like the
/// other ports of the control plane. An adapter honours `timeout` and `limit` of the request, and
/// follows no redirect; any status is an answer, not an error.
pub trait Transport {
    /// What sending fails with.
    type Error: std::error::Error + 'static;

    /// Sends `request`.
    fn send(&self, request: Request) -> impl Future<Output = Result<Response, Self::Error>>;
}

/// A transport with its limits: the one thing an authority sends through.
#[derive(Debug)]
pub struct Client<T> {
    transport: T,
    limits: Limits,
}

impl<T: Transport> Client<T> {
    /// A client over `transport` that stamps `limits` on every request.
    pub const fn new(transport: T, limits: Limits) -> Self {
        Self { transport, limits }
    }

    /// The transport.
    pub const fn transport(&self) -> &T {
        &self.transport
    }

    /// The limits.
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Sends `request` within the limits, and refuses a reply larger than they allow.
    pub async fn send(&self, mut request: Request) -> Result<Response, Fail<T::Error>> {
        request.timeout = self.limits.timeout;
        request.limit = self.limits.reply;
        let response = self.transport.send(request).await.map_err(Fail::Transport)?;
        if response.body.len() > self.limits.reply {
            return Err(Fail::Oversize { size: response.body.len(), limit: self.limits.reply });
        }
        Ok(response)
    }

    /// Sends `request` and reads its 2xx reply as JSON. Any other status is [`Fail::Status`].
    pub async fn fetch<R: DeserializeOwned>(&self, request: Request) -> Result<R, Fail<T::Error>> {
        let response = self.send(request).await?;
        if !response.success() {
            return Err(Fail::Status { status: response.status });
        }
        response.json().map_err(Fail::Json)
    }
}

/// The fake was told to fail, or asked for a route it was not given.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Faked {
    /// Told to fail.
    Down,
    /// No reply was given for this call.
    Unrouted {
        /// What it did.
        method: Method,
        /// Where.
        url: String,
    },
}

impl fmt::Display for Faked {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Down => out.write_str("the fake transport was told to fail"),
            Self::Unrouted { method, url } => write!(out, "the fake transport has no reply for {method:?} {url}"),
        }
    }
}

impl std::error::Error for Faked {}

#[derive(Debug)]
struct Route {
    method: Method,
    url: String,
    replies: VecDeque<Response>,
}

/// A [`Transport`] that answers from replies it was given, and keeps what it was asked.
#[derive(Debug, Default)]
pub struct Fake {
    routes: RefCell<Vec<Route>>,
    sent: RefCell<Vec<Request>>,
    down: Cell<bool>,
}

impl Fake {
    /// A fake with no replies.
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers `method` `url` with `reply` from now on, in place of any reply given before.
    pub fn reply(&self, method: Method, url: &str, reply: Response) {
        let mut routes = self.routes.borrow_mut();
        match routes.iter_mut().find(|route| route.method == method && route.url == url) {
            Some(route) => route.replies = VecDeque::from([reply]),
            None => routes.push(Route { method, url: url.to_owned(), replies: VecDeque::from([reply]) }),
        }
    }

    /// Answers `method` `url` with `reply` after the replies given before have been used once
    /// each. The last reply repeats.
    pub fn then(&self, method: Method, url: &str, reply: Response) {
        let mut routes = self.routes.borrow_mut();
        match routes.iter_mut().find(|route| route.method == method && route.url == url) {
            Some(route) => route.replies.push_back(reply),
            None => routes.push(Route { method, url: url.to_owned(), replies: VecDeque::from([reply]) }),
        }
    }

    /// Makes every call fail, or answer again.
    pub fn fail(&self, down: bool) {
        self.down.set(down);
    }

    /// Takes the requests sent so far, oldest first.
    pub fn taken(&self) -> Vec<Request> {
        std::mem::take(&mut self.sent.borrow_mut())
    }
}

impl Transport for Fake {
    type Error = Faked;

    fn send(&self, request: Request) -> impl Future<Output = Result<Response, Faked>> {
        let answer = if self.down.get() {
            Err(Faked::Down)
        } else {
            let mut routes = self.routes.borrow_mut();
            let route =
                routes.iter_mut().find(|route| route.method == request.method && route.url == request.url.as_str());
            match route {
                Some(route) if route.replies.len() > 1 => route.replies.pop_front().ok_or(Faked::Down),
                Some(route) => route.replies.front().cloned().ok_or(Faked::Down),
                None => Err(Faked::Unrouted { method: request.method, url: request.url.to_string() }),
            }
        };
        self.sent.borrow_mut().push(request);
        std::future::ready(answer)
    }
}
