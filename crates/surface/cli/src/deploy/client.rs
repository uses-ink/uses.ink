//! A typed client of the edge API (`edge/api`): routes under `/v1`, every call signed with the
//! machine's key (`Ink-Sig-1`, decision 0068).
//!
//! One connection per call, HTTP/1.1 over a plain socket. There is no TLS in this tree, so an
//! `https://` address is refused with a typed error rather than sent in the clear.

use std::time::Duration;

use build::BuildId;
use control::{DomainName, Id, SiteKind};
use edge_api::Probed;
use edge_api::view::{BuildView, DeployView, DomainView, Listed, Moved, Status as Overview, Unclaimed};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{ACCEPT, AUTHORIZATION, CONNECTION, CONTENT_TYPE, HOST};
use hyper::{Method, Request, Uri};
use hyper_util::rt::TokioIo;
use model::DeployId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;

use super::failure::{ApiRefusal, Failure};
use crate::command::auth::Identity;

/// The body of a rollback request.
#[derive(Debug, Serialize)]
struct Target {
    /// The deploy to make live.
    deploy: DeployId,
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

    /// The authority as the client writes it in `Host`, and as a signature names the audience.
    pub(crate) fn authority(&self) -> &str {
        &self.authority
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

/// How a call that is safe to repeat is repeated when the network fails or the server is not
/// there: how many more times, and how long to wait before the first, doubling after each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Retry {
    /// How many more attempts after the first.
    pub(crate) times: u32,
    /// The wait before the first more.
    pub(crate) wait: Duration,
}

impl Retry {
    /// No second attempt.
    pub(crate) const NONE: Self = Self { times: 0, wait: Duration::ZERO };
}

/// The way to the API: where it is, who calls and how long a call may take. Every call is
/// signed with the key, and sent again once when the server says it has just started.
#[derive(Debug)]
pub(crate) struct Wire {
    pub(crate) endpoint: Endpoint,
    /// The key to sign with. `None` for the one call nobody signs: asking to enrol.
    pub(crate) identity: Option<Identity>,
    /// How long one call may take.
    pub(crate) budget: Duration,
    /// How a read, or a write that can be sent twice, is repeated.
    pub(crate) retry: Retry,
}

/// The calls of one site.
#[derive(Debug)]
pub(crate) struct Api {
    pub(crate) wire: Wire,
    pub(crate) site: Id<SiteKind>,
}

impl Api {
    /// `GET /v1/sites/{site}`: the site, what is live, the latest build and the pending ones.
    pub(crate) async fn overview(&self) -> Result<Overview, Failure> {
        let body = self.wire.repeat(Method::GET, &format!("/v1/sites/{}", self.site), Vec::new()).await?;
        serde_json::from_slice(&body).map_err(|source| Failure::Unreadable { source })
    }

    /// `POST /v1/sites/{site}/source`: the tree of the folder, answered with the blobs the site
    /// lacks or the build of it. Sending the same tree twice is safe, so it is repeated.
    pub(crate) async fn probe(&self, tree: Vec<u8>) -> Result<Probed, Failure> {
        let body = self.wire.repeat(Method::POST, &self.path("source"), tree).await?;
        serde_json::from_slice(&body).map_err(|source| Failure::Unreadable { source })
    }

    /// `PUT /v1/sites/{site}/source`: a tar archive of blobs named by their hashes.
    pub(crate) async fn parts(&self, batch: Vec<u8>) -> Result<(), Failure> {
        self.wire.repeat(Method::PUT, &self.path("source"), batch).await.map(drop)
    }

    /// `GET /v1/sites/{site}/source`: the files of the latest push, as a tar archive.
    pub(crate) async fn bundle(&self) -> Result<Bytes, Failure> {
        self.wire.repeat(Method::GET, &self.path("source"), Vec::new()).await
    }

    /// `GET /v1/sites/{site}/export`, or of one deploy: the files it serves, as a tar archive.
    pub(crate) async fn export(&self, deploy: Option<DeployId>) -> Result<Bytes, Failure> {
        let tail = deploy.map_or_else(|| "export".to_owned(), |deploy| format!("deploys/{deploy}/export"));
        self.wire.repeat(Method::GET, &self.path(&tail), Vec::new()).await
    }

    /// `GET /v1/sites/{site}/deploys`: the history, newest first.
    pub(crate) async fn deploys(&self) -> Result<Vec<DeployView>, Failure> {
        let items: Listed<DeployView> = self.look("deploys").await?;
        Ok(items.items)
    }

    /// `GET /v1/sites/{site}/builds`: recent builds, newest first.
    pub(crate) async fn builds(&self) -> Result<Vec<BuildView>, Failure> {
        let items: Listed<BuildView> = self.look("builds").await?;
        Ok(items.items)
    }

    /// The path of `tail` under the site.
    fn path(&self, tail: &str) -> String {
        format!("/v1/sites/{}/{tail}", self.site)
    }

    /// A read of the site's `tail`, repeated when the network fails.
    async fn look<T: DeserializeOwned>(&self, tail: &str) -> Result<T, Failure> {
        let body = self.wire.repeat(Method::GET, &self.path(tail), Vec::new()).await?;
        serde_json::from_slice(&body).map_err(|source| Failure::Unreadable { source })
    }

    /// `GET /v1/sites/{site}/builds/{build}/log`: the lines of the log, `None` when it has none.
    pub(crate) async fn log(&self, build: BuildId) -> Result<Option<Vec<String>>, Failure> {
        match self.wire.repeat(Method::GET, &self.path(&format!("builds/{build}/log")), Vec::new()).await {
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

    /// `GET /v1/sites/{site}/domains`: the names that serve the site, then the ones to prove.
    pub(crate) async fn domains(&self) -> Result<Vec<DomainView>, Failure> {
        let items: Listed<DomainView> = self.look("domains").await?;
        Ok(items.items)
    }

    /// `PUT /v1/sites/{site}/domains/{name}`: asks to prove the name; the answer is the record.
    pub(crate) async fn claim(&self, name: &DomainName) -> Result<DomainView, Failure> {
        self.read(Method::PUT, &format!("domains/{name}"), b"{}".to_vec()).await
    }

    /// `POST /v1/sites/{site}/domains/{name}/proof`: the server reads the record and decides.
    pub(crate) async fn prove(&self, name: &DomainName) -> Result<DomainView, Failure> {
        self.read(Method::POST, &format!("domains/{name}/proof"), b"{}".to_vec()).await
    }

    /// `DELETE /v1/sites/{site}/domains/{name}`.
    pub(crate) async fn unclaim(&self, name: &DomainName) -> Result<Unclaimed, Failure> {
        self.read(Method::DELETE, &format!("domains/{name}"), Vec::new()).await
    }

    /// A call whose answer is the JSON `T`.
    async fn read<T: DeserializeOwned>(&self, method: Method, tail: &str, body: Vec<u8>) -> Result<T, Failure> {
        let answer = self.send(method, tail, body).await?;
        serde_json::from_slice(&answer).map_err(|source| Failure::Unreadable { source })
    }

    /// One call of the site, within the budget; the body of an answer that is a success.
    async fn send(&self, method: Method, tail: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        self.wire.call(method, &format!("/v1/sites/{}/{tail}", self.site), body).await
    }
}

impl Wire {
    /// A call that is safe to send twice, sent again after a failure of the network or of the
    /// server's own, as often as [`Retry`] says, each wait twice the one before. A refusal that is
    /// the caller's is never sent again.
    pub(crate) async fn repeat(&self, method: Method, path: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        let mut wait = self.retry.wait;
        let mut left = self.retry.times;
        loop {
            match self.call(method.clone(), path, body.clone()).await {
                Err(failure) if left > 0 && failure.transient() => {
                    tokio::time::sleep(wait).await;
                    wait = wait.saturating_mul(2);
                    left -= 1;
                }
                done => return done,
            }
        }
    }

    /// One call, within the budget, and once more when the server has just started and says so.
    pub(crate) async fn call(&self, method: Method, path: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        let first = self.once(method.clone(), path, body.clone()).await;
        let Err(Failure::Refused { status: 503, refusal: Some(refusal) }) = &first else { return first };
        if refusal.problem != "auth.server.restarting" {
            return first;
        }
        let seconds = refusal.fields.get("seconds").and_then(|text| text.parse().ok()).unwrap_or(1);
        tokio::time::sleep(Duration::from_secs(seconds).min(self.budget)).await;
        self.once(method, path, body).await
    }

    /// One attempt, within the budget.
    async fn once(&self, method: Method, path: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        match tokio::time::timeout(self.budget, self.exchange(method, path, body)).await {
            Ok(answer) => answer,
            Err(_) => Err(Failure::Timeout { after: self.budget }),
        }
    }

    /// The exchange itself: sign, connect, send one request, read the whole answer.
    async fn exchange(&self, method: Method, path: &str, body: Vec<u8>) -> Result<Bytes, Failure> {
        let signed = match &self.identity {
            Some(identity) => Some(identity.sign(method.as_str(), path, &body)?),
            None => None,
        };
        let address = self.endpoint.address();
        let stream = TcpStream::connect(&address).await.map_err(|source| Failure::Connect { address, source })?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .map_err(|source| Failure::Exchange { source })?;
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(HOST, &self.endpoint.authority)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .header(CONNECTION, "close");
        if let Some(signed) = signed {
            request = request.header(AUTHORIZATION, signed);
        }
        let request = request.body(Full::new(Bytes::from(body))).map_err(|source| Failure::Request { source })?;
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
            Err(Failure::Refused { status, refusal: serde_json::from_slice::<ApiRefusal>(&body).ok() })
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
    fn an_identity_does_not_print_its_key() {
        let seed = sig::Seed::new([5; 32]);
        let identity = Identity::new(&seed, "api.localhost:8080").unwrap();
        let shown = format!("{identity:?}");
        assert!(shown.starts_with("Identity(") && !shown.contains(seed.text().reveal()), "{shown}");
    }
}
