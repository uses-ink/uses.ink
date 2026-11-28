//! A Worker's request in, the API's reply out: the part of the edge's bridge a Worker needs, with
//! the body bounded by what the route may cost.

use edge_api::{Answers, Limits, Media, Method, Payload, Reply, Request, Status};
use futures_util::StreamExt as _;
use worker::{Headers, Response};

/// The API as a Worker's `fetch` calls it.
#[derive(Debug)]
pub struct Calls<A> {
    api: A,
    limits: Limits,
}

impl<A: Answers> Calls<A> {
    /// `api` answers the calls, whose bodies `limits` bound.
    pub const fn new(api: A, limits: Limits) -> Self {
        Self { api, limits }
    }

    /// The answerer.
    pub const fn api(&self) -> &A {
        &self.api
    }

    /// Answers `incoming`: reads at most what its route may cost of the body, asks the API, and
    /// writes the reply.
    pub async fn respond(&self, incoming: &mut worker::Request) -> worker::Result<Response> {
        let Ok(method) = incoming.method().to_string().parse::<Method>() else {
            return Self::response(&Reply::empty(Status::MethodNotAllowed));
        };
        let url = incoming.url()?;
        let path = url.path().to_owned();
        let limit = self.limits.budget(method, &path);
        let headers = incoming.headers();
        let text = |name: &str| headers.get(name).ok().flatten();
        let request = Request {
            method,
            query: url.query().map(str::to_owned),
            authorization: text("authorization").map(String::into_bytes),
            cookie: text("cookie").map(String::into_bytes),
            origin: text("origin"),
            site: text("sec-fetch-site"),
            headers: Self::hook_headers(headers, &path),
            source: text("cf-connecting-ip").and_then(|address| address.parse().ok()),
            body: Self::read(incoming, limit).await?,
            path,
        };
        Self::response(&self.api.answer(&request).await)
    }

    /// The headers a forge's webhook is read by, and none for any other route.
    fn hook_headers(headers: &Headers, path: &str) -> Vec<(String, String)> {
        if !path.starts_with(edge_api::HOOKS) {
            return Vec::new();
        }
        headers.entries().filter(|(name, _)| !matches!(name.as_str(), "authorization" | "cookie")).collect()
    }

    /// The body, up to `limit` bytes: more is `Over`, and what was sent past it is not read.
    async fn read(incoming: &mut worker::Request, limit: u64) -> worker::Result<Payload> {
        let told = incoming.headers().get("content-length")?.and_then(|length| length.parse::<u64>().ok());
        if told.is_some_and(|length| length > limit) {
            return Ok(Payload::Over { limit });
        }
        // a request with no body has no stream to read, and says so by neither header
        let chunked = incoming.headers().get("transfer-encoding")?.is_some();
        if told.unwrap_or(0) == 0 && !chunked {
            return Ok(Payload::Whole(Vec::new()));
        }
        let mut stream = incoming.stream()?;
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            body.extend_from_slice(&chunk?);
            if u64::try_from(body.len()).unwrap_or(u64::MAX) > limit {
                return Ok(Payload::Over { limit });
            }
        }
        Ok(Payload::Whole(body))
    }

    /// A reply as a response: never cached, never sniffed, and sandboxed, as the edge's bridge
    /// writes it.
    fn response(reply: &Reply) -> worker::Result<Response> {
        let headers = Headers::new();
        for cookie in reply.cookie.iter().chain(&reply.ceremony) {
            headers.append("set-cookie", &cookie.to_string())?;
        }
        headers.set("cache-control", "no-store")?;
        headers.set("x-content-type-options", "nosniff")?;
        headers.set("content-security-policy", "default-src 'none'; frame-ancestors 'none'; sandbox")?;
        headers.set("cross-origin-resource-policy", "same-origin")?;
        if let Some(kind) = reply.media.content_type() {
            headers.set("content-type", kind)?;
        }
        if reply.challenge {
            headers.set("www-authenticate", "Ink-Sig-1")?;
        }
        if let Some(retry) = reply.retry {
            headers.set("retry-after", &retry.as_secs().to_string())?;
        }
        for header in &reply.headers {
            headers.set(header.name, &header.value)?;
        }
        if !reply.allow.is_empty() {
            let allow: Vec<String> = reply.allow.iter().map(ToString::to_string).collect();
            headers.set("allow", &allow.join(", "))?;
        }
        let body = if reply.media == Media::Empty { Vec::new() } else { reply.body.clone() };
        Ok(Response::from_bytes(body)?.with_status(reply.status.code()).with_headers(headers))
    }
}
