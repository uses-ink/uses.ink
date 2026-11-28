//! One bounded https request to an API: the client a forge is asked through (decision 0162).

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use reqwest::header::{HeaderName, HeaderValue};
use reqwest::redirect::Policy;

use crate::{Https, Misconfigured};

/// What a request does. A forge call here reads or creates; none deletes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verb {
    /// Reads.
    Get,
    /// Creates.
    Post,
}

/// A name that is sent to an address of the operator's choosing instead of the one DNS gives: a
/// forge behind a mirror, or a test. The port is the address's own; the certificate is still
/// checked for the name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// The DNS name, as the URL writes it.
    pub host: String,
    /// Where to connect instead.
    pub address: SocketAddr,
}

/// One request: where, what it carries and how much of the answer is read. Its `Debug` names the
/// headers and never prints their values, since one is a credential.
#[derive(Clone, Copy)]
pub struct Call<'a> {
    /// What it does.
    pub verb: Verb,
    /// The `https` address.
    pub url: &'a str,
    /// Header names and values, which are not text when a credential holds odd bytes.
    pub headers: &'a [(&'a str, &'a [u8])],
    /// The body, or nothing.
    pub body: &'a [u8],
    /// How long the whole call may take, the answer's body included.
    pub timeout: Duration,
    /// How many bytes of the answer are kept: one more is read, so a caller can tell that the
    /// answer was longer.
    pub limit: usize,
}

impl fmt::Debug for Call<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| *name).collect();
        out.debug_struct("Call")
            .field("verb", &self.verb)
            .field("url", &self.url)
            .field("headers", &names)
            .field("body", &format_args!("{} bytes", self.body.len()))
            .field("timeout", &self.timeout)
            .field("limit", &self.limit)
            .finish()
    }
}

/// What was answered. Any status is an answer. Its `Debug` shows the status and the size, never the
/// body.
#[derive(Clone, PartialEq, Eq)]
pub struct Answer {
    /// The status code.
    pub status: u16,
    /// The body, at most one byte over the call's limit.
    pub body: Vec<u8>,
}

impl fmt::Debug for Answer {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Answer {{ status: {}, body: <{} bytes> }}", self.status, self.body.len())
    }
}

/// Why a request got no answer.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failed {
    /// A header name or value is not one HTTP can carry.
    Header {
        /// The header's name.
        name: String,
    },
    /// The request was not sent or the answer did not start: the address does not resolve, the
    /// connection or the certificate was refused, the address is not `https`, or the time ran out.
    Send(reqwest::Error),
    /// The answer began and its body could not be read to the end.
    Read(reqwest::Error),
}

impl fmt::Display for Failed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header { name } => write!(out, "the header `{name}` cannot be sent"),
            Self::Send(_) => out.write_str("the request got no answer"),
            Self::Read(_) => out.write_str("the answer's body could not be read"),
        }
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Header { .. } => None,
            Self::Send(source) | Self::Read(source) => Some(source),
        }
    }
}

/// The HTTP client of an API call: the one rustls configuration of [`Https`], https only, HTTP/1.1,
/// no proxy and no redirect, so a `3xx` is the answer and nothing is followed to where the forge
/// did not say it lives.
#[derive(Clone, Debug)]
pub struct Fetch {
    client: reqwest::Client,
}

impl Fetch {
    /// A client over the roots, the connect timeout and the extra roots of `https`, sending each
    /// name of `routes` to its address.
    pub fn new(https: &Https, routes: &[Route]) -> Result<Self, Misconfigured> {
        let mut builder = reqwest::Client::builder()
            .tls_backend_preconfigured(https.client()?)
            .https_only(true)
            .http1_only()
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(https.connect);
        for route in routes {
            builder = builder.resolve(&route.host, route.address);
        }
        Ok(Self { client: builder.build().map_err(Misconfigured::Client)? })
    }

    /// Sends `call` and reads the answer up to its limit.
    pub async fn send(&self, call: Call<'_>) -> Result<Answer, Failed> {
        let mut request = match call.verb {
            Verb::Get => self.client.get(call.url),
            Verb::Post => self.client.post(call.url),
        };
        for (name, value) in call.headers {
            let header = || Failed::Header { name: (*name).to_owned() };
            let mut held = HeaderValue::from_bytes(value).map_err(|_| header())?;
            held.set_sensitive(true);
            request = request.header(HeaderName::from_bytes(name.as_bytes()).map_err(|_| header())?, held);
        }
        if call.verb == Verb::Post {
            request = request.body(call.body.to_vec());
        }
        let mut response = request.timeout(call.timeout).send().await.map_err(Failed::Send)?;
        let status = response.status().as_u16();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(Failed::Read)? {
            let room = call.limit.saturating_add(1).saturating_sub(body.len());
            body.extend_from_slice(chunk.get(..room.min(chunk.len())).unwrap_or_default());
            if body.len() > call.limit {
                break;
            }
        }
        Ok(Answer { status, body })
    }
}
