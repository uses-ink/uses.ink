//! In-memory ports and a small site, for the edge's tests.

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "shared by several test targets, which stop at the first surprise"
)]

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;

use bytes::Bytes;
use edge::{
    Blob, BlobError, Blobs, Body, Chunks, Deploy, Edge, Encoding, Limits, Name, Reply, Request, Resolved, Scheme,
    Sites, Span,
};
use model::{Hash, Header, HeaderName, HeaderValue, Manifest, MediaType, Route, Served, Status};

/// The one thing the stand-in sites can get wrong.
#[derive(Debug, PartialEq, Eq)]
pub struct Broken;

impl std::fmt::Display for Broken {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("the stand-in store is broken")
    }
}

impl std::error::Error for Broken {}

/// Hosts held in memory.
#[derive(Debug, Default)]
pub struct Hosts(pub BTreeMap<String, Arc<Deploy>>, pub bool);

impl Sites for Hosts {
    type Scope = ();
    type Error = Broken;

    fn resolve(&self, hostname: &edge::Hostname) -> impl Future<Output = Result<Option<Resolved<()>>, Broken>> + Send {
        let broken = self.1;
        let found = self.0.get(hostname.as_str()).cloned();
        async move {
            if broken {
                return Err(Broken);
            }
            Ok(found.map(|deploy| Resolved { deploy, scope: () }))
        }
    }
}

/// Blobs held in memory.
#[derive(Debug, Default)]
pub struct Store(pub BTreeMap<Hash, Vec<u8>>);

#[derive(Debug)]
pub struct Held(Bytes);

#[derive(Debug)]
pub struct Whole(Option<Bytes>);

impl Blobs for Store {
    type Scope = ();
    type Blob = Held;
    type Error = Infallible;

    fn open(&self, _: &(), hash: Hash) -> impl Future<Output = Result<Held, BlobError<Infallible>>> + Send {
        let found = self.0.get(&hash).map(|bytes| Bytes::from(bytes.clone()));
        async move { found.map(Held).ok_or(BlobError::Missing { hash }) }
    }
}

impl Blob for Held {
    type Body = Whole;

    fn length(&self) -> u64 {
        self.0.len() as u64
    }

    fn read(self, span: Span) -> Whole {
        let start = usize::try_from(span.start).unwrap();
        let end = start + usize::try_from(span.length).unwrap();
        Whole(Some(self.0.slice(start..end)))
    }
}

impl Chunks for Whole {
    type Error = Infallible;

    fn next(&mut self) -> impl Future<Output = Result<Option<Bytes>, Infallible>> + Send {
        let next = self.0.take();
        async move { Ok(next) }
    }
}

/// The file a site's media route holds: bytes `0..=255` repeated.
pub fn media() -> Vec<u8> {
    (0..1000u32).map(|n| (n % 256) as u8).collect()
}

/// The page policy the engine writes into a page's headers, in miniature.
pub const PAGE_CSP: &str = "default-src 'none'; img-src 'self'";

/// A site of `notes.example`: a home, a page, a stylesheet with variants, a media file, the 404
/// page, and `/old` moved to `/thé`.
pub struct Site {
    pub hosts: Hosts,
    pub store: Store,
}

impl Site {
    pub fn new() -> Self {
        let mut store = Store::default();
        let mut put = |bytes: &[u8]| {
            let hash = Hash::of(bytes);
            store.0.insert(hash, bytes.to_vec());
            hash
        };
        let page = |route: &str, hash: Hash, status: Status| {
            let csp = Header::new(HeaderName::ContentSecurityPolicy, HeaderValue::parse(PAGE_CSP).unwrap());
            Served::new(Route::parse(route).unwrap(), hash, MediaType::Html, vec![csp]).with_status(status)
        };
        let css = put(b"body { color: black }");
        let css_br = put(b"br bytes");
        let css_gzip = put(b"gzip bytes");
        let immutable =
            Header::new(HeaderName::CacheControl, HeaderValue::parse("public, max-age=31536000, immutable").unwrap());
        let entries = vec![
            page("/", put(b"<h1>home</h1>"), Status::Ok),
            page("/tea", put(b"<h1>tea</h1>"), Status::Ok),
            page("/404", put(b"<h1>nothing here</h1>"), Status::NotFound),
            Served::new(Route::parse("/style.css").unwrap(), css, MediaType::Css, vec![immutable]),
            Served::new(Route::parse("/clip.png").unwrap(), put(&media()), MediaType::Png, vec![]),
            Served::new(Route::parse("/empty.png").unwrap(), put(b""), MediaType::Png, vec![]),
        ];
        let deploy = Deploy {
            manifest: Manifest::new(entries).unwrap(),
            moved: [(Route::parse("/old").unwrap(), Route::parse("/thé").unwrap())].into_iter().collect(),
            variants: [(css, Encoding::Brotli, css_br), (css, Encoding::Gzip, css_gzip)].into_iter().collect(),
        };
        let mut hosts = Hosts::default();
        hosts.0.insert("notes.example".to_owned(), Arc::new(deploy));
        let bare =
            Deploy { manifest: Manifest::new(vec![page("/", put(b"bare"), Status::Ok)]).unwrap(), ..Deploy::default() };
        hosts.0.insert("bare.example".to_owned(), Arc::new(bare));
        Self { hosts, store }
    }

    pub fn edge(self) -> Edge<Hosts, Store> {
        Edge::new(self.hosts, self.store, Limits::default()).unwrap().with_probe("/healthz".parse().unwrap())
    }
}

/// A response read to its end.
#[derive(Debug)]
pub struct Got {
    pub status: u16,
    pub fields: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Got {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields.iter().find(|(field, _)| field == name).map(|(_, value)| value.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8(self.body.clone()).unwrap()
    }
}

/// Runs a future to its end on a runtime that has no timers and no I/O.
pub fn block<F: Future>(future: F) -> F::Output {
    thread_local! {
        static RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
    }
    RUNTIME.with(|runtime| runtime.block_on(future))
}

/// Reads a reply's body to its end.
pub async fn read(reply: Reply<Hosts, Store>) -> (Got, edge::Outcome<edge::Fault<Broken, Infallible>>) {
    let edge::Answer { response, outcome, .. } = reply;
    let fields = response.fields.iter().map(|f| (f.name.to_string(), f.value.to_string())).collect();
    let mut body = Vec::new();
    match response.body {
        Body::Empty => {}
        Body::Text(text) => body.extend(text.into_bytes()),
        Body::Blob { blob, span } => {
            let mut chunks = blob.read(span);
            while let Some(chunk) = chunks.next().await.unwrap() {
                body.extend_from_slice(&chunk);
            }
        }
        _ => unreachable!("a body kind the tests do not know"),
    }
    (Got { status: response.status.code(), fields, body }, outcome)
}

/// A request over plain HTTP with `host`, plus these headers.
pub fn get<'a>(method: &'a str, target: &'a str, headers: &[(&'a str, &'a str)]) -> Request<'a> {
    let mut request = Request::new(method, target, Scheme::Plain);
    request.extend(headers.iter().map(|(name, value)| (name.as_bytes(), value.as_bytes())));
    request
}

/// What `edge` answers to `request`, read to its end.
pub fn answer(
    edge: &Edge<Hosts, Store>,
    request: &Request<'_>,
) -> (Got, edge::Outcome<edge::Fault<Broken, Infallible>>) {
    block(async { read(edge.respond(request).await).await })
}

pub fn has_name(got: &Got, name: Name) -> bool {
    got.field(name.name()).is_some()
}
