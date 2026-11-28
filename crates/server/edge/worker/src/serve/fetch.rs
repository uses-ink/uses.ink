//! A Worker's request in, the edge's answer out.

use edge::{Blobs, Body, Edge, Field, Request, Scheme, Sites, Status};
use futures_util::stream;
use worker::{Headers, Response};

/// What a request got: the response and a line to say how it went.
#[derive(Debug)]
pub struct Answered {
    /// The response to send.
    pub response: Response,
    /// The status the edge chose.
    pub status: Status,
}

/// The edge as a Worker's `fetch` calls it.
#[derive(Debug)]
pub struct Serve<S, B>
where
    S: Sites,
    B: Blobs<Scope = S::Scope>,
{
    edge: Edge<S, B>,
}

impl<S, B> Serve<S, B>
where
    S: Sites,
    B: Blobs<Scope = S::Scope>,
    <B::Blob as edge::Blob>::Body: 'static,
{
    /// Serves through `edge`.
    pub const fn new(edge: Edge<S, B>) -> Self {
        Self { edge }
    }

    /// The edge.
    pub const fn edge(&self) -> &Edge<S, B> {
        &self.edge
    }

    /// Answers `request`. The platform has already parsed the URL and merged the header lines of a
    /// name, so the refusals of raw targets (dot segments, a doubled `Host` or `Range`) never meet a
    /// request that holds them; the rest of the edge's checks apply as they do to any adapter.
    pub async fn respond(&self, request: &worker::Request) -> worker::Result<Answered> {
        let url = request.url()?;
        let target = url.query().map_or_else(|| url.path().to_owned(), |query| format!("{}?{query}", url.path()));
        let authority = url.port().map_or_else(
            || url.host_str().unwrap_or_default().to_owned(),
            |port| format!("{}:{port}", url.host_str().unwrap_or_default()),
        );
        let mut lines: Vec<(String, String)> =
            request.headers().entries().filter(|(name, _)| !name.eq_ignore_ascii_case("host")).collect();
        lines.push(("host".to_owned(), authority));
        let scheme = if url.scheme() == "https" { Scheme::Tls } else { Scheme::Plain };
        let method = request.method().to_string();
        let mut asked = Request::new(&method, &target, scheme);
        asked.extend(lines.iter().map(|(name, value)| (name.as_bytes(), value.as_bytes())));
        let answer = self.edge.respond(&asked).await;
        let status = answer.response.status;
        let headers = Headers::new();
        for Field { name, value } in &answer.response.fields {
            headers.append(name.name(), value.as_str())?;
        }
        let built = match answer.response.body {
            Body::Blob { blob, span } => {
                let chunks = edge::Blob::read(blob, span);
                let body = stream::unfold(chunks, |mut chunks| async move {
                    match edge::Chunks::next(&mut chunks).await {
                        Ok(Some(chunk)) => Some((Ok(chunk.to_vec()), chunks)),
                        Ok(None) => None,
                        Err(error) => Some((Err(worker::Error::RustError(error.to_string())), chunks)),
                    }
                });
                Response::from_stream(body)?
            }
            Body::Text(text) => Response::from_bytes(text.into_bytes())?,
            _ => Response::empty()?,
        };
        let response = built.with_status(status.code()).with_headers(headers);
        Ok(Answered { response, status })
    }
}
