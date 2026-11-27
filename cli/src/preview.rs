//! `ink preview <dir>`: the site built in memory and served on 127.0.0.1 with the manifest's
//! statuses and headers, through axum on a one-thread tokio runtime.

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Request, Response, StatusCode, header};
use engine::Blobs;
use host_native::Memory;
use model::{Manifest, Route};
use serde::Serialize;

use crate::args;
use crate::cache::Caching;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::{Address, Loaded, Rendering, Source};

/// What `ink preview --json` prints before it serves: the address, or none when the site has
/// errors and nothing is served.
#[derive(Debug, Serialize)]
struct Served {
    schema: &'static str,
    address: Option<String>,
    errors: usize,
    warnings: usize,
}

/// A deploy held in memory, ready to serve.
#[derive(Clone, Debug)]
pub struct Preview {
    manifest: Arc<Manifest>,
    blobs: Arc<Memory>,
}

impl args::Preview {
    /// Binds the port first, so the site is built for the address it is served at: with
    /// `--port 0`, the port the system chose.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let serve = |source| Error::Serve { source };
        let listener = std::net::TcpListener::bind(("127.0.0.1", self.port)).map_err(serve)?;
        let port = listener.local_addr().map_err(serve)?.port();
        let address = format!("http://127.0.0.1:{port}");
        let address = Address { chosen: None, ..Address::from_flag(Some(&address))? };
        let loaded = Source::new(&self.dir, &self.origin).load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let caching = Caching::new(&self.dir, self.cache.as_deref());
        let mut rendered = loaded.render(&address, &caching.pixels());
        rendered.notices.extend(caching.finish());
        let tally = io.said(&rendered.diagnostics, &rendered.notices, texts)?;
        let preview = Preview::assemble(loaded, rendered);
        io.json(&Served {
            schema: "uses.preview/v1",
            address: preview.as_ref().map(|_| address.base.to_string()),
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        let Some(preview) = preview else { return Ok(Exit::Content) };
        io.line(&address.base)?;
        preview.serve(listener)?;
        Ok(tally.exit(Strict::Off))
    }
}

impl Preview {
    /// The site in `dir` built for `base`, or `None` when it has errors. What the build said is
    /// not reported: `ink preview` reports it. Nothing is kept between builds: the work is all
    /// in memory.
    pub fn of(dir: &Path, base: &str) -> Result<Option<Self>, Error> {
        let address = Address::from_flag(Some(base))?;
        let loaded = Source::new(dir, &args::Origin::default()).load()?;
        let rendered = loaded.render(&address, &host_image::Pixels::default());
        Ok(Self::assemble(loaded, rendered))
    }

    fn assemble(loaded: Loaded, rendered: Rendering) -> Option<Self> {
        let errors = rendered.diagnostics.iter().any(|d| d.severity() == model::Severity::Error);
        let manifest = rendered.deploy.filter(|_| !errors)?;
        let mut blobs = loaded.store;
        blobs.extend(rendered.blobs.into_values());
        Some(Self { manifest: Arc::new(manifest), blobs: Arc::new(blobs) })
    }

    /// Serves on `listener` until the process ends.
    pub fn serve(self, listener: std::net::TcpListener) -> Result<(), Error> {
        let serve = |source| Error::Serve { source };
        listener.set_nonblocking(true).map_err(serve)?;
        let runtime = tokio::runtime::Builder::new_current_thread().enable_io().build().map_err(serve)?;
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).map_err(serve)?;
            let router = Router::new().fallback(move |request: Request<Body>| {
                let preview = self.clone();
                async move { preview.respond(request.uri().path()) }
            });
            axum::serve(listener, router).await.map_err(serve)
        })
    }

    /// The deploy as the edge would serve it: an entry with its status and headers, or the
    /// not-found page with 404.
    fn respond(&self, path: &str) -> Response<Body> {
        let decoded = Decoded::try_from(path).ok();
        let path = decoded.as_ref().map_or("", |decoded| decoded.0.as_str());
        let trimmed = if path.len() > 1 { path.trim_end_matches('/') } else { path };
        let route: Option<Route> = trimmed.parse().ok();
        let found = route.as_ref().and_then(|route| self.manifest.entry(route));
        let not_found = || "/404".parse::<Route>().ok().and_then(|route| self.manifest.entry(&route));
        match found.or_else(not_found) {
            Some(entry) => self.entry(entry, found.is_some()),
            None => Self::status(StatusCode::NOT_FOUND),
        }
    }

    fn entry(&self, entry: &model::Served, found: bool) -> Response<Body> {
        let Ok(bytes) = self.blobs.read(&entry.blob()) else { return Self::status(StatusCode::INTERNAL_SERVER_ERROR) };
        let status = if found { entry.status().code() } else { StatusCode::NOT_FOUND.as_u16() };
        let mut response = Response::new(Body::from(bytes.into_owned()));
        *response.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let headers = response.headers_mut();
        if let Ok(value) = HeaderValue::from_str(&entry.media().to_string()) {
            headers.insert(header::CONTENT_TYPE, value);
        }
        for served in entry.headers() {
            if let (Ok(name), Ok(value)) =
                (HeaderName::from_bytes(served.name.name().as_bytes()), HeaderValue::from_str(served.value.as_str()))
            {
                headers.append(name, value);
            }
        }
        response
    }

    fn status(status: StatusCode) -> Response<Body> {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = status;
        response
    }
}

/// A request's path, percent-decoded as UTF-8: `/th%C3%A9` is `/thé`, the route it names.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Decoded(String);

/// Why a request's path does not decode: it names no route, so it is served as not found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Undecodable {
    /// A `%` not followed by two hex digits, at this byte offset.
    Escape { at: usize },
    /// The decoded bytes are not UTF-8.
    Encoding,
}

impl TryFrom<&str> for Decoded {
    type Error = Undecodable;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        let bytes = path.as_bytes();
        let mut decoded = Vec::with_capacity(bytes.len());
        let mut at = 0;
        while let Some(&byte) = bytes.get(at) {
            if byte != b'%' {
                decoded.push(byte);
                at += 1;
                continue;
            }
            let digits = bytes.get(at + 1..at + 3).filter(|digits| digits.iter().all(u8::is_ascii_hexdigit));
            let value = digits
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 16).ok())
                .ok_or(Undecodable::Escape { at })?;
            decoded.push(value);
            at += 3;
        }
        String::from_utf8(decoded).map(Self).map_err(|_| Undecodable::Encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::{Decoded, Undecodable};

    #[test]
    fn decodes_percent_escapes_as_utf8() {
        assert_eq!(Decoded::try_from("/th%C3%A9"), Ok(Decoded("/thé".to_owned())));
        assert_eq!(Decoded::try_from("/thé"), Ok(Decoded("/thé".to_owned())));
        assert_eq!(Decoded::try_from("/a%2"), Err(Undecodable::Escape { at: 2 }));
        assert_eq!(Decoded::try_from("/a%+1"), Err(Undecodable::Escape { at: 2 }));
        assert_eq!(Decoded::try_from("/%C3"), Err(Undecodable::Encoding));
    }
}
