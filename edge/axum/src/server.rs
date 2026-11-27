//! The server: accept, hand to the edge, stream the answer.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::Response;
use edge::{Blob, Blobs, Body as Payload, Caller, Edge, Outcome, Refusal, Scheme, Sites, Stage};
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use hyper_util::service::TowerToHyperService;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::body::Pump;
use crate::bridge::Mount;
use crate::error::Error;
use crate::event::{Answered, Called, Event, Kind, Observe, Stopped};
use crate::respond::{Code, Fields};

/// What the smallest head hyper reads is.
const LEAST_HEAD: u64 = 8192;

/// How long an accept loop rests after the system refuses a connection (out of descriptors), so a
/// full table is not a busy loop.
const REST: Duration = Duration::from_millis(100);

/// The edge, served over HTTP/1 on a listener.
#[derive(Debug)]
pub struct Server<S: Sites, B: Blobs<Scope = S::Scope>, O> {
    inner: Arc<Inner<S, B, O>>,
}

#[derive(Debug)]
struct Inner<S: Sites, B: Blobs<Scope = S::Scope>, O> {
    edge: Edge<S, B>,
    observe: Arc<O>,
    gate: Arc<Semaphore>,
    scheme: Scheme,
    api: Option<Mount>,
}

impl<S, B, O> Server<S, B, O>
where
    S: Sites + 'static,
    B: Blobs<Scope = S::Scope> + 'static,
    B::Blob: 'static,
    <B::Blob as Blob>::Body: 'static,
    O: Observe,
{
    /// Serves `edge`, telling `observe` of everything, for clients that arrive over `scheme`
    /// (what terminates TLS in front of this server says so: HSTS follows it).
    pub fn new(edge: Edge<S, B>, observe: O, scheme: Scheme) -> Result<Self, Error> {
        let limit = edge.limits().head.count();
        if limit < LEAST_HEAD {
            return Err(Error::Head { limit, least: LEAST_HEAD });
        }
        let edge = edge.with_probe("/healthz".parse().map_err(Error::Probe)?);
        let gate = Arc::new(Semaphore::new(edge.limits().connections));
        Ok(Self { inner: Arc::new(Inner { edge, observe: Arc::new(observe), gate, scheme, api: None }) })
    }

    /// Serves the API at `mount`: a request whose `Host` is the mount's host is an API call and
    /// never reaches the edge, so no author's route is shadowed.
    #[must_use]
    pub fn with_api(mut self, mount: Mount) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.api = Some(mount);
        }
        self
    }

    /// The axum router: the edge is its one route.
    pub fn router(&self) -> Router {
        Router::new().fallback(Inner::answer).with_state(Arc::clone(&self.inner))
    }

    /// Serves one accepted connection under hyper's head and read budgets, until it ends or the
    /// shutdown drains it.
    fn connect(&self, graceful: &GracefulShutdown, router: Router, stream: TcpStream) {
        let limits = self.inner.edge.limits();
        let service = TowerToHyperService::new(router);
        let mut http = http1::Builder::new();
        http.timer(TokioTimer::new())
            .header_read_timeout(limits.read)
            .max_buf_size(usize::try_from(limits.head.count()).unwrap_or(usize::MAX))
            .max_headers(limits.headers);
        let connection = graceful.watch(http.serve_connection(TokioIo::new(stream), service));
        let observe = Arc::clone(&self.inner.observe);
        tokio::spawn(async move {
            if let Err(source) = connection.await {
                observe.event(&Event::Connection { source: Box::new(source) });
            }
        });
    }

    /// Accepts connections on `listener` until `shutdown` completes, then stops accepting and
    /// gives open connections [`Limits::drain`] to finish.
    pub async fn run(self, listener: TcpListener, shutdown: impl Future<Output = ()> + Send) -> Result<(), Error> {
        let address = listener.local_addr().map_err(Error::Address)?;
        let observe = &self.inner.observe;
        let limits = *self.inner.edge.limits();
        observe.event(&Event::Listening { address });
        let router = self.router();
        let graceful = GracefulShutdown::new();
        let mut shutdown = pin!(shutdown);
        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => self.connect(&graceful, router.clone(), stream),
                    Err(source) => {
                        observe.event(&Event::Connection { source: Box::new(source) });
                        tokio::time::sleep(REST).await;
                    }
                },
                () = &mut shutdown => break,
            }
        }
        observe.event(&Event::Stopping);
        let stopped = tokio::select! {
            () = graceful.shutdown() => Stopped::Drained,
            () = tokio::time::sleep(limits.drain) => Stopped::Abandoned { limit: limits.drain },
        };
        observe.event(&Event::Stopped(stopped));
        Ok(())
    }
}

/// What a request said before the edge saw it: kept for the event.
#[derive(Debug)]
struct Seen {
    method: String,
    target: String,
    started: Instant,
}

impl<S, B, O> Inner<S, B, O>
where
    S: Sites + 'static,
    B: Blobs<Scope = S::Scope> + 'static,
    B::Blob: 'static,
    <B::Blob as Blob>::Body: 'static,
    O: Observe,
{
    /// One request: within the concurrency limit, within the lookup budget, answered by the edge.
    async fn answer(State(inner): State<Arc<Self>>, request: Request) -> Response<Body> {
        if let Some(mount) = inner.api.as_ref().filter(|mount| Self::names(mount, &request)) {
            return inner.call(mount, request).await;
        }
        let seen = Seen {
            method: request.method().as_str().to_owned(),
            target: request.uri().to_string(),
            started: Instant::now(),
        };
        let mut asked = edge::Request::new(&seen.method, &seen.target, inner.scheme);
        asked.extend(request.headers().iter().map(|(name, value)| (name.as_str().as_bytes(), value.as_bytes())));
        let limits = *inner.edge.limits();
        let caller = Caller { hostname: None, scheme: inner.scheme };
        let (permit, reply) = match Arc::clone(&inner.gate).try_acquire_owned() {
            Err(_) => (None, inner.edge.refuse(Refusal::Busy { limit: limits.connections }, &caller)),
            Ok(permit) => match tokio::time::timeout(limits.open, inner.edge.respond(&asked)).await {
                Ok(reply) => (Some(permit), reply),
                Err(_) => (None, inner.edge.refuse(Refusal::Slow { stage: Stage::Open, limit: limits.open }, &caller)),
            },
        };
        inner.send(reply, &seen, permit)
    }

    /// Whether the request's `Host` is the API's: the name before any port, in lower case.
    fn names(mount: &Mount, request: &Request) -> bool {
        let host = request.headers().get("host").and_then(|value| value.to_str().ok()).unwrap_or_default();
        let name = host
            .rsplit_once(':')
            .filter(|(_, port)| port.bytes().all(|b| b.is_ascii_digit()))
            .map_or(host, |(name, _)| name);
        name.eq_ignore_ascii_case(&mount.host)
    }

    /// One API call, and the event that says it was made.
    async fn call(&self, mount: &Mount, request: Request) -> Response<Body> {
        let started = Instant::now();
        let method = request.method().as_str().to_owned();
        let path: String = request.uri().path().chars().take(200).collect();
        let response = mount.bridge.call(request).await;
        let status = response.status().as_u16();
        self.observe.event(&Event::Called(Called { method, path, status, took: started.elapsed() }));
        response
    }

    /// The body of a response: a blob streamed under the idle and send budgets, or its text.
    fn pump(&self, payload: Payload<B::Blob>, seen: &Seen, permit: Option<tokio::sync::OwnedSemaphorePermit>) -> Body {
        let limits = self.edge.limits();
        match payload {
            Payload::Blob { blob, span } => Pump {
                chunks: blob.read(span),
                idle: limits.idle,
                send: limits.send,
                deadline: seen.started + limits.send,
                sent: 0,
                observe: Arc::clone(&self.observe),
                permit,
            }
            .body(),
            Payload::Text(text) => Body::from(text),
            _ => Body::empty(),
        }
    }

    /// The edge's reply as a response, and the event that says what happened.
    fn send(
        &self,
        reply: edge::Reply<S, B>,
        seen: &Seen,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Response<Body> {
        let edge::Answer { response, outcome, hostname } = reply;
        let mut kind = Kind::from(&outcome);
        let (refusal, mut fault) = match outcome {
            Outcome::Refused(refusal) => (Some(refusal), None),
            Outcome::Failed(fault) => (None, Some(Box::new(fault) as Box<dyn std::error::Error + Send + Sync>)),
            _ => (None, None),
        };
        let body = self.pump(response.body, seen, permit);
        let mut built = Response::new(body);
        let mut status = response.status;
        if let Err(malformed) = Fields(&response.fields).put(&mut built) {
            status = edge::Status::Internal;
            kind = Kind::Failed;
            fault = Some(Box::new(malformed));
        }
        *built.status_mut() = Code(status).into();
        self.observe.event(&Event::Answered(Answered {
            method: seen.method.clone(),
            path: seen.target.split('?').next().unwrap_or_default().chars().take(200).collect(),
            hostname,
            status,
            kind,
            took: seen.started.elapsed(),
            refusal,
            fault,
        }));
        built
    }
}
