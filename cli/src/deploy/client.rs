//! A typed client of the edge API (`edge/api`): routes under `/v1/sites`, a bearer token.
//!
//! One connection per call, HTTP/1.1 over a plain socket. There is no TLS in this tree, so an
//! `https://` address is refused with a typed error rather than sent in the clear.

use std::fmt;
use std::time::Duration;

use control::{Id, SiteKind};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{ACCEPT, AUTHORIZATION, CONNECTION, CONTENT_TYPE, HOST};
use hyper::{Method, Request, Uri};
use hyper_util::rt::TokioIo;
use model::DeployId;
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;

use super::failure::{Failure, Said};
use super::wire::{Built, Items, Moved, Row, Target};

/// The owner token. It never prints.
#[derive(Clone)]
pub(crate) struct Token(pub(crate) String);

impl fmt::Debug for Token {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Token(..)")
    }
}

/// Where the API is: a host and port, and what to put in `Host`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Endpoint {
    authority: String,
}

impl Endpoint {
    /// The endpoint `written` names: `http://host[:port]`, nothing after it but a slash.
    pub(crate) fn parse(written: &str) -> Result<Self, Failure> {
        let uri: Uri = written.parse().map_err(|source| Failure::Address { written: written.to_owned(), source })?;
        let plain = uri.scheme_str() == Some("http");
        let bare = uri.path_and_query().is_none_or(|path| path.as_str() == "/");
        match (uri.authority(), plain && bare) {
            (Some(authority), true) => Ok(Self { authority: authority.to_string() }),
            _ => Err(Failure::Scheme { written: written.to_owned() }),
        }
    }

    /// `host:port` as a socket address wants it: port 80 when none is given.
    fn address(&self) -> String {
        if self.authority.rsplit_once(':').is_some_and(|(_, port)| port.chars().all(|c| c.is_ascii_digit())) {
            self.authority.clone()
        } else {
            format!("{}:80", self.authority)
        }
    }
}

/// The calls of one site.
#[derive(Debug)]
pub(crate) struct Api {
    pub(crate) endpoint: Endpoint,
    pub(crate) token: Token,
    pub(crate) site: Id<SiteKind>,
    /// How long one call may take.
    pub(crate) budget: Duration,
}

impl Api {
    /// `GET /v1/sites/{site}/deploys`: the history, newest first.
    pub(crate) async fn deploys(&self) -> Result<Vec<Row>, Failure> {
        let items: Items<Row> = self.read(Method::GET, "deploys", Vec::new()).await?;
        Ok(items.items)
    }

    /// `GET /v1/sites/{site}/builds`: recent builds, newest first.
    pub(crate) async fn builds(&self) -> Result<Vec<Built>, Failure> {
        let items: Items<Built> = self.read(Method::GET, "builds", Vec::new()).await?;
        Ok(items.items)
    }

    /// `GET /v1/sites/{site}/builds/{build}/log`: the lines of the log, `None` when it has none.
    pub(crate) async fn log(&self, build: &str) -> Result<Option<Vec<String>>, Failure> {
        match self.send(Method::GET, &format!("builds/{build}/log"), Vec::new()).await {
            Ok(body) => Ok(Some(String::from_utf8_lossy(&body).lines().map(str::to_owned).collect())),
            Err(Failure::Refused { status: 404, .. }) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// `POST /v1/sites/{site}/rollback`.
    pub(crate) async fn rollback(&self, deploy: DeployId) -> Result<Moved, Failure> {
        let body = serde_json::to_vec(&Target { deploy }).map_err(|source| Failure::Unreadable { source })?;
        self.read(Method::POST, "rollback", body).await
    }

    /// `POST /v1/sites/{site}/resume`.
    pub(crate) async fn resume(&self) -> Result<Moved, Failure> {
        self.read(Method::POST, "resume", b"{}".to_vec()).await
    }

    /// A call whose answer is the JSON `T`.
    async fn read<T: DeserializeOwned>(&self, method: Method, tail: &str, body: Vec<u8>) -> Result<T, Failure> {
        let answer = self.send(method, tail, body).await?;
        serde_json::from_slice(&answer).map_err(|source| Failure::Unreadable { source })
    }

    /// One call, within the budget; the body of an answer that is a success.
    async fn send(&self, method: Method, tail: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        let path = format!("/v1/sites/{}/{tail}", self.site);
        match tokio::time::timeout(self.budget, self.exchange(method, &path, body)).await {
            Ok(answer) => answer,
            Err(_) => Err(Failure::Timeout { after: self.budget }),
        }
    }

    /// The exchange itself: connect, send one request, read the whole answer.
    async fn exchange(&self, method: Method, path: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        let address = self.endpoint.address();
        let stream = TcpStream::connect(&address).await.map_err(|source| Failure::Connect { address, source })?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|source| Failure::Exchange { source })?;
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(HOST, &self.endpoint.authority)
            .header(AUTHORIZATION, format!("Bearer {}", self.token.0))
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .header(CONNECTION, "close")
            .body(Full::new(Bytes::from(body)))
            .map_err(|source| Failure::Request { source })?;
        let asked = async move {
            let response = sender.send_request(request).await?;
            let status = response.status().as_u16();
            let body = response.into_body().collect().await?.to_bytes();
            Ok::<_, hyper::Error>((status, body))
        };
        let (answer, closed) = tokio::join!(asked, connection);
        let (status, body) = answer.map_err(|source| Failure::Exchange { source })?;
        closed.map_err(|source| Failure::Exchange { source })?;
        if (200..300).contains(&status) {
            Ok(body)
        } else {
            Err(Failure::Refused { status, said: serde_json::from_slice::<Said>(&body).ok() })
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn an_endpoint_is_plain_http_with_a_host() {
        assert_eq!(
            Endpoint::parse("http://api.localhost:8080").map(|e| e.address()).ok().as_deref(),
            Some("api.localhost:8080")
        );
        assert_eq!(
            Endpoint::parse("http://api.localhost/").map(|e| e.address()).ok().as_deref(),
            Some("api.localhost:80")
        );
        assert!(matches!(Endpoint::parse("https://api.uses.ink"), Err(Failure::Scheme { .. })));
        assert!(matches!(Endpoint::parse("http://a/v1"), Err(Failure::Scheme { .. })));
        assert!(matches!(Endpoint::parse("api.uses.ink"), Err(Failure::Scheme { .. })));
    }

    #[test]
    fn the_token_does_not_print() {
        assert_eq!(format!("{:?}", Token("secret".into())), "Token(..)");
    }
}
