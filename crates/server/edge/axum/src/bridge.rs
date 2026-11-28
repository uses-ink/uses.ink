//! The API's way into the server: a channel from the connection tasks to the one task that owns
//! the control plane.
//!
//! The control plane's ports are not `Send` (D1 and R2 on Workers are not), and a connection
//! task is spawned, so it cannot hold them. [`Bridge::new`] makes two halves: a [`Bridge`] that
//! is `Send` and clone, which the server calls, and a [`Local`] that holds the API and answers
//! one call at a time on whatever task the caller runs it on (a `LocalSet`, or the main task
//! beside the server).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Request as Incoming, Response};
use edge_api::{Answers, Limits, Method, Payload, Reply, Request, Status};
use tokio::sync::{mpsc, oneshot};

use crate::forward::Forwarded;

/// Where a connection came from, put in the request's extensions by the server so the API can
/// limit by source.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Peer(pub(crate) Option<std::net::SocketAddr>);

/// How many calls wait for the control plane before more are refused.
const WAITING: usize = 64;

/// A call on its way to the API task, with where to send the reply.
#[derive(Debug)]
struct Call {
    request: Request,
    reply: oneshot::Sender<Reply>,
}

/// Where the API is served: the host that names it, and the half that sends it calls.
#[derive(Clone, Debug)]
pub struct Mount {
    /// The host name (without a port) whose requests are API calls, such as `api.localhost`.
    pub host: Arc<str>,
    /// The way to the API.
    pub bridge: Bridge,
}

/// The `Send` half: takes a call and gets its reply.
#[derive(Clone, Debug)]
pub struct Bridge {
    calls: mpsc::Sender<Call>,
    limits: Limits,
    forwarded: Forwarded,
}

/// The half that holds the API.
#[derive(Debug)]
pub struct Local<A> {
    calls: mpsc::Receiver<Call>,
    api: A,
}

impl Bridge {
    /// The two halves over `api`, whose body limits are `limits`.
    pub fn new<A: Answers>(api: A, limits: Limits) -> (Self, Local<A>) {
        let (calls, received) = mpsc::channel(WAITING);
        (Self { calls, limits, forwarded: Forwarded::Off }, Local { calls: received, api })
    }

    /// The same bridge with a rule for finding the client behind a proxy. The default is
    /// [`Forwarded::Off`].
    #[must_use]
    pub const fn forwarded(mut self, forwarded: Forwarded) -> Self {
        self.forwarded = forwarded;
        self
    }

    /// Answers one HTTP request to the API.
    pub async fn call(&self, incoming: Incoming<Body>) -> Response<Body> {
        let (head, body) = incoming.into_parts();
        let method = head.method.as_str().parse::<Method>();
        let Ok(method) = method else { return Self::plain(Status::MethodNotAllowed) };
        let path = head.uri.path().to_owned();
        let query = head.uri.query().map(str::to_owned);
        let limit = self.limits.budget(method, &path);
        let authorization = head.headers.get("authorization").map(|value| value.as_bytes().to_vec());
        let cookie = head.headers.get("cookie").map(|value| value.as_bytes().to_vec());
        let origin = head.headers.get("origin").and_then(|value| value.to_str().ok()).map(str::to_owned);
        let site = head.headers.get("sec-fetch-site").and_then(|value| value.to_str().ok()).map(str::to_owned);
        let peer = head.extensions.get::<Peer>().and_then(|peer| peer.0).map(|address| address.ip().to_canonical());
        let source = match self.forwarded.client(peer, &head.headers) {
            Ok(source) => source,
            Err(refused) => {
                return Self::response(&Reply::new(Status::BadRequest, edge_api::Media::Json, refused.body()));
            }
        };
        let read = axum::body::to_bytes(body, usize::try_from(limit).unwrap_or(usize::MAX));
        let body = match tokio::time::timeout(std::time::Duration::from_secs(600), read).await {
            Err(_) => return Self::plain(Status::Timeout),
            Ok(Ok(bytes)) => Payload::Whole(bytes.to_vec()),
            Ok(Err(_)) => Payload::Over { limit },
        };
        let (reply, answered) = oneshot::channel();
        let request = Request { method, path, query, authorization, cookie, origin, site, source, body };
        if self.calls.try_send(Call { request, reply }).is_err() {
            return Self::plain(Status::Internal);
        }
        match answered.await {
            Ok(reply) => Self::response(&reply),
            Err(_) => Self::plain(Status::Internal),
        }
    }

    fn plain(status: Status) -> Response<Body> {
        Self::response(&Reply::empty(status))
    }

    /// A reply as an HTTP response: never cached, never sniffed, and sandboxed.
    fn response(reply: &Reply) -> Response<Body> {
        let mut response = Response::new(Body::from(reply.body.clone()));
        *response.status_mut() = axum::http::StatusCode::from_u16(reply.status.code()).unwrap_or_default();
        let headers = response.headers_mut();
        for cookie in reply.cookie.iter().chain(&reply.ceremony) {
            if let Ok(value) = HeaderValue::from_str(&cookie.to_string()) {
                headers.append(HeaderName::from_static("set-cookie"), value);
            }
        }
        let mut put = |name: &'static str, value: &str| {
            if let Ok(value) = HeaderValue::from_str(value) {
                headers.insert(HeaderName::from_static(name), value);
            }
        };
        put("cache-control", "no-store");
        put("x-content-type-options", "nosniff");
        put("content-security-policy", "default-src 'none'; frame-ancestors 'none'; sandbox");
        put("cross-origin-resource-policy", "same-origin");
        if let Some(kind) = reply.media.content_type() {
            put("content-type", kind);
        }
        if reply.challenge {
            put("www-authenticate", "Ink-Sig-1");
        }
        if let Some(retry) = reply.retry {
            put("retry-after", &retry.as_secs().to_string());
        }
        if !reply.allow.is_empty() {
            let allow: Vec<String> = reply.allow.iter().map(ToString::to_string).collect();
            put("allow", &allow.join(", "));
        }
        response
    }
}

impl<A: Answers> Local<A> {
    /// Answers calls one at a time until every [`Bridge`] is dropped.
    pub async fn run(mut self) {
        while let Some(Call { request, reply }) = self.calls.recv().await {
            // a caller that went away no longer wants its answer
            reply.send(self.api.answer(&request).await).ok();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
mod test {
    use std::num::NonZeroU8;

    use super::*;

    /// An API that answers with the source it was told, as the body.
    struct Echo;

    impl Answers for Echo {
        fn answer(&self, request: &Request) -> impl std::future::Future<Output = Reply> {
            let said = request.source.map_or_else(String::new, |source| source.to_string());
            std::future::ready(Reply::new(Status::Ok, edge_api::Media::Json, said.into_bytes()))
        }
    }

    async fn ask(rule: Forwarded, header: Option<&str>) -> (u16, String) {
        let (bridge, local) = Bridge::new(Echo, Limits::default());
        let bridge = bridge.forwarded(rule);
        let mut builder = Incoming::builder().method("GET").uri("/v1/sessions");
        if let Some(header) = header {
            builder = builder.header("x-forwarded-for", header);
        }
        let mut incoming = builder.body(Body::empty()).unwrap();
        incoming.extensions_mut().insert(Peer(Some("10.0.0.1:4000".parse().unwrap())));
        // the bridge is moved in and dropped when the call is answered, which ends `Local::run`
        let call = async move { bridge.call(incoming).await };
        let (response, ()) = tokio::join!(call, local.run());
        let status = response.status().as_u16();
        let body = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn the_source_is_the_connection_unless_a_proxy_is_trusted() {
        assert_eq!(ask(Forwarded::Off, Some("1.2.3.4")).await, (200, "10.0.0.1".to_owned()));
        let one = Forwarded::Hops(NonZeroU8::new(1).unwrap());
        assert_eq!(ask(one, Some("9.9.9.9, 203.0.113.7")).await, (200, "203.0.113.7".to_owned()));
        let (status, body) = ask(one, None).await;
        assert_eq!(status, 400);
        assert!(body.contains("auth.forwarded.refused"), "{body}");
    }
}
