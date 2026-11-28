//! The transport that reaches a forge over https (decision 0162).

use std::fmt;

use authority_forge::{Host, Method, Request, Response, Transport};
use https::{Call, Failed, Fetch, Verb};

/// Why a call was not sent, or got no answer.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unwired {
    /// The address is not `https`.
    Scheme {
        /// What it was.
        scheme: String,
    },
    /// The address names a host the operator did not list. A server never sends a call to a name a
    /// stranger chose.
    Unlisted {
        /// The host, or nothing for an address with none.
        host: String,
    },
    /// The call was sent and got no answer.
    Fetch(Failed),
}

impl fmt::Display for Unwired {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheme { scheme } => write!(out, "a forge call goes over https, not `{scheme}`"),
            Self::Unlisted { host } => write!(out, "`{host}` is not a host the operator listed for forge calls"),
            Self::Fetch(_) => out.write_str("the forge did not answer"),
        }
    }
}

impl std::error::Error for Unwired {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fetch(source) => Some(source),
            Self::Scheme { .. } | Self::Unlisted { .. } => None,
        }
    }
}

/// A forge transport over [`Fetch`]: https only, to hosts on a list, no redirect, and the limits of
/// the request it is given.
#[derive(Debug)]
pub struct Wire {
    fetch: Fetch,
    hosts: Vec<Host>,
    agent: String,
}

impl Wire {
    /// A transport that sends through `fetch` to `hosts` and introduces itself as `agent`, which
    /// GitHub requires.
    pub const fn new(fetch: Fetch, hosts: Vec<Host>, agent: String) -> Self {
        Self { fetch, hosts, agent }
    }

    /// Whether `name` is a listed host, compared as DNS compares.
    fn listed(&self, name: &str) -> bool {
        self.hosts.iter().any(|host| host.as_str().eq_ignore_ascii_case(name))
    }
}

impl Transport for Wire {
    type Error = Unwired;

    async fn send(&self, request: Request) -> Result<Response, Unwired> {
        let url = &request.url;
        if url.scheme() != "https" {
            return Err(Unwired::Scheme { scheme: url.scheme().to_owned() });
        }
        let name = url.host_str().unwrap_or_default();
        if !self.listed(name) {
            return Err(Unwired::Unlisted { host: name.to_owned() });
        }
        let bearer = request.bearer.as_ref().map(|secret| [b"Bearer ", secret.reveal()].concat());
        let mut headers: Vec<(&str, &[u8])> = vec![("User-Agent", self.agent.as_bytes())];
        if let Some(bearer) = &bearer {
            headers.push(("Authorization", bearer));
        }
        if !request.body.is_empty() {
            headers.push(("Content-Type", b"application/json"));
        }
        for (name, value) in &request.headers {
            headers.push((name, value.as_bytes()));
        }
        let verb = match request.method {
            Method::Get => Verb::Get,
            Method::Post => Verb::Post,
        };
        let call = Call {
            verb,
            url: url.as_str(),
            headers: &headers,
            body: &request.body,
            timeout: request.timeout,
            limit: request.limit,
        };
        let answer = self.fetch.send(call).await.map_err(Unwired::Fetch)?;
        Ok(Response::new(answer.status, answer.body))
    }
}
