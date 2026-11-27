//! The edge over real sockets: pages, conditionals, ranges, hostile requests as bytes on the wire,
//! slow clients, the concurrency limit and a graceful stop.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]
extern crate uses_ink_edge_axum as edge_axum;

#[path = "../../tests/support/mod.rs"]
mod support;

use std::convert::Infallible;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use edge::{Blob, BlobError, Blobs, Chunks, Edge, Limits, Scheme, Span};
use edge_axum::{Event, Kind, Observe, Server};
use model::Hash;
use support::{Hosts, Site, Store};

/// Events as they came.
#[derive(Debug, Default)]
struct Recorder(Mutex<Vec<Event>>);

/// The recorder, shared with the thread the server runs on.
struct Shared(Arc<Recorder>);

impl Observe for Shared {
    fn event(&self, event: &Event) {
        // `Event` is not `Clone` (it holds boxed errors): the recorder keeps the parts a test reads
        self.0.0.lock().unwrap().push(match event {
            Event::Answered(answered) => Event::Answered(edge_axum::Answered {
                method: answered.method.clone(),
                path: answered.path.clone(),
                hostname: answered.hostname.clone(),
                status: answered.status,
                kind: answered.kind,
                took: answered.took,
                refusal: answered.refusal.clone(),
                fault: None,
            }),
            Event::Cut(cut) => Event::Cut(edge_axum::Cut { stage: cut.stage, limit: cut.limit, sent: cut.sent }),
            Event::Stopping => Event::Stopping,
            Event::Stopped(stopped) => Event::Stopped(*stopped),
            Event::Listening { address } => Event::Listening { address: *address },
            _ => return,
        });
    }
}

/// A running server on a thread of its own.
struct Running {
    address: SocketAddr,
    events: Arc<Recorder>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<Result<(), edge_axum::Error>>>,
}

impl Running {
    fn start<B>(hosts: Hosts, blobs: B, limits: Limits) -> Self
    where
        B: Blobs<Scope = ()> + 'static,
        B::Blob: 'static,
        <B::Blob as Blob>::Body: 'static,
    {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let events = Arc::new(Recorder::default());
        let server =
            Server::new(Edge::new(hosts, blobs, limits).unwrap(), Shared(Arc::clone(&events)), Scheme::Plain).unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                server
                    .run(listener, async {
                        stopped.await.ok();
                    })
                    .await
            })
        });
        Self { address, events, stop: Some(stop), thread: Some(thread) }
    }

    fn site() -> Self {
        let site = Site::new();
        Self::start(site.hosts, site.store, Limits::default())
    }

    /// Sends `bytes` and reads until the server closes.
    fn raw(&self, bytes: &[u8]) -> String {
        let mut stream = TcpStream::connect(self.address).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        // `Connection: close` after the request line, so the server ends the exchange
        let text = String::from_utf8_lossy(bytes).replacen("\r\n", "\r\nConnection: close\r\n", 1);
        stream.write_all(text.as_bytes()).unwrap();
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);
        String::from_utf8_lossy(&response).into_owned()
    }

    fn get(&self, target: &str, headers: &str) -> String {
        self.raw(
            format!("GET {target} HTTP/1.1\r\nHost: notes.example\r\nConnection: close\r\n{headers}\r\n").as_bytes(),
        )
    }

    fn stop(&mut self) -> Result<(), edge_axum::Error> {
        if let Some(stop) = self.stop.take() {
            stop.send(()).ok();
        }
        self.thread.take().unwrap().join().unwrap()
    }

    fn kinds(&self) -> Vec<Kind> {
        self.events
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| if let Event::Answered(a) = e { Some(a.kind) } else { None })
            .collect()
    }
}

fn status(response: &str) -> &str {
    response.split(' ').nth(1).unwrap_or_default()
}

fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response.lines().find_map(|line| {
        let (field, value) = line.split_once(": ")?;
        field.eq_ignore_ascii_case(name).then_some(value)
    })
}

fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").map_or("", |(_, body)| body)
}

#[test]
fn serves_pages_conditionals_and_ranges_over_a_socket() {
    let mut server = Running::site();
    let page = server.get("/tea", "");
    assert_eq!(status(&page), "200");
    assert_eq!(body(&page), "<h1>tea</h1>");
    assert_eq!(header(&page, "x-content-type-options"), Some("nosniff"));
    assert_eq!(header(&page, "content-length"), Some("12"));
    let etag = header(&page, "etag").unwrap().to_owned();
    assert_eq!(status(&server.get("/tea", &format!("If-None-Match: {etag}\r\n"))), "304");
    let ranged = server.get("/clip.png", "Range: bytes=10-14\r\n");
    assert_eq!(status(&ranged), "206");
    assert_eq!(header(&ranged, "content-range"), Some("bytes 10-14/1000"));
    assert_eq!(body(&ranged).len(), 5);
    assert_eq!(status(&server.get("/clip.png", "Range: bytes=5000-\r\n")), "416");
    let missing = server.get("/nope", "");
    assert_eq!((status(&missing), body(&missing)), ("404", "<h1>nothing here</h1>"));
    let head = server.raw(b"HEAD /clip.png HTTP/1.1\r\nHost: notes.example\r\nConnection: close\r\n\r\n");
    assert_eq!((status(&head), header(&head, "content-length"), body(&head)), ("200", Some("1000"), ""));
    assert_eq!(status(&server.get("/old", "")), "301");
    let probe = server.raw(b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    assert_eq!((status(&probe), body(&probe)), ("200", "ok"));
    let kinds = server.kinds();
    assert!(
        kinds.contains(&Kind::Served) && kinds.contains(&Kind::Unchanged) && kinds.contains(&Kind::Probed),
        "{kinds:?}"
    );
    assert!(server.stop().is_ok());
}

#[test]
fn hostile_requests_on_the_wire_are_refused_and_logged() {
    let mut server = Running::site();
    let big = "a".repeat(9000);
    let cases: Vec<(&str, String, &str)> = vec![
        ("traversal", "GET /../etc/passwd HTTP/1.1\r\nHost: notes.example\r\n\r\n".into(), "400"),
        ("escaped traversal", "GET /..%2f..%2fetc/passwd HTTP/1.1\r\nHost: notes.example\r\n\r\n".into(), "400"),
        ("double encoding", "GET /%252e%252e/ HTTP/1.1\r\nHost: notes.example\r\n\r\n".into(), "400"),
        ("protocol-relative", "GET //evil.example/ HTTP/1.1\r\nHost: notes.example\r\n\r\n".into(), "400"),
        ("encoded nul", "GET /a%00 HTTP/1.1\r\nHost: notes.example\r\n\r\n".into(), "400"),
        ("two hosts", "GET / HTTP/1.1\r\nHost: notes.example\r\nHost: evil.example\r\n\r\n".into(), "400"),
        ("no host", "GET / HTTP/1.1\r\n\r\n".into(), "400"),
        ("host with path", "GET / HTTP/1.1\r\nHost: notes.example/x\r\n\r\n".into(), "400"),
        ("absolute form mismatch", "GET http://evil.example/ HTTP/1.1\r\nHost: notes.example\r\n\r\n".into(), "400"),
        ("post", "POST / HTTP/1.1\r\nHost: notes.example\r\nContent-Length: 0\r\n\r\n".into(), "405"),
        ("get with a body", "GET / HTTP/1.1\r\nHost: notes.example\r\nContent-Length: 4\r\n\r\nabcd".into(), "400"),
        (
            "chunked get",
            "GET / HTTP/1.1\r\nHost: notes.example\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".into(),
            "400",
        ),
        (
            "length and chunked",
            "GET / HTTP/1.1\r\nHost: notes.example\r\nContent-Length: 4\r\nTransfer-Encoding: chunked\r\n\r\n".into(),
            "400",
        ),
        ("overlong target", format!("GET /{big} HTTP/1.1\r\nHost: notes.example\r\n\r\n"), "414"),
        ("huge header", format!("GET / HTTP/1.1\r\nHost: notes.example\r\nX: {big}\r\n\r\n"), "431"),
        ("unknown host", "GET / HTTP/1.1\r\nHost: nobody.example\r\n\r\n".into(), "404"),
    ];
    for (name, request, expected) in cases {
        let response = server.raw(request.as_bytes());
        assert_eq!(status(&response), expected, "{name}: {response}");
        assert!(!body(&response).contains("root:"), "{name}");
    }
    let events = server.events.0.lock().unwrap();
    let refused = events.iter().filter(|event| matches!(event, Event::Answered(a) if a.refusal.is_some())).count();
    assert!(refused >= 12, "the edge logged {refused} refusals as typed events");
    drop(events);
    assert!(server.stop().is_ok());
}

#[test]
fn a_client_that_never_finishes_its_head_is_cut_off() {
    let site = Site::new();
    let limits = Limits { read: Duration::from_millis(50), ..Limits::default() };
    let mut server = Running::start(site.hosts, site.store, limits);
    let mut stream = TcpStream::connect(server.address).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.write_all(b"GET / HTTP/1.1\r\nHost: notes.exa").unwrap();
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response);
    assert!(text.is_empty() || status(&text) == "408", "{text}");
    assert!(server.stop().is_ok());
}

#[test]
fn over_the_concurrency_limit_is_a_503() {
    let site = Site::new();
    let limits = Limits { connections: 0, ..Limits::default() };
    let mut server = Running::start(site.hosts, site.store, limits);
    let response = server.get("/tea", "");
    assert_eq!(status(&response), "503");
    assert_eq!(header(&response, "retry-after"), Some("1"));
    assert_eq!(header(&response, "x-content-type-options"), Some("nosniff"));
    assert_eq!(server.kinds(), [Kind::Refused]);
    assert!(server.stop().is_ok());
}

#[test]
fn a_head_limit_hyper_cannot_read_is_refused_at_start() {
    let site = Site::new();
    let limits = Limits { head: model::Bytes(100), ..Limits::default() };
    let edge = Edge::new(site.hosts, site.store, limits).unwrap();
    let error = Server::new(edge, edge_axum::Tracing, Scheme::Plain).unwrap_err();
    assert!(matches!(error, edge_axum::Error::Head { limit: 100, least: 8192 }));
}

/// A blob whose bytes never come: a reader that stopped taking, seen from the other side.
#[derive(Debug)]
struct Stalled;

#[derive(Debug)]
struct Waiting;

impl Blobs for Stalled {
    type Scope = ();
    type Blob = Waiting;
    type Error = Infallible;

    async fn open(&self, (): &(), _: Hash) -> Result<Waiting, BlobError<Infallible>> {
        Ok(Waiting)
    }
}

impl Blob for Waiting {
    type Body = Waiting;

    fn length(&self) -> u64 {
        1000
    }

    fn read(self, _: Span) -> Waiting {
        self
    }
}

impl Chunks for Waiting {
    type Error = Infallible;

    async fn next(&mut self) -> Result<Option<Bytes>, Infallible> {
        std::future::pending().await
    }
}

#[test]
fn a_response_that_stops_flowing_is_cut_and_logged() {
    let limits = Limits { idle: Duration::from_millis(50), ..Limits::default() };
    let mut server = Running::start(Site::new().hosts, Stalled, limits);
    let response = server.get("/tea", "");
    assert_eq!(status(&response), "200");
    assert_eq!(body(&response), "", "the body never came and the connection was closed");
    let events = server.events.0.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(e, Event::Cut(cut) if cut.stage == edge::Stage::Idle && cut.sent == 0)),
        "{events:?}"
    );
    drop(events);
    assert!(server.stop().is_ok());
}

#[test]
fn stops_when_told_and_says_so() {
    let mut server = Running::site();
    assert_eq!(status(&server.get("/tea", "")), "200");
    assert!(server.stop().is_ok());
    let events = server.events.0.lock().unwrap();
    assert!(matches!(events.first(), Some(Event::Listening { .. })));
    assert!(matches!(events.last(), Some(Event::Stopped(edge_axum::Stopped::Drained))));
    assert!(TcpStream::connect(server.address).is_err(), "nothing listens after a stop");
    let _ = &Store::default();
}
