//! Proving a public repository by the repo challenge, with no App and no credential.

use authority_forge::{Challenge, Client, Fail, Host, Method, Proven, Repo, Request, Transport, Unproven};

/// The media type that asks GitHub for a file's own bytes.
const RAW: &str = "application/vnd.github.raw+json";
/// The version of the REST API this is written against.
const VERSION: &str = "2022-11-28";

/// The GitHub proof of the repo challenge (decision 0111): the same file as the other forges, read
/// from the contents API with no credential, so it works for a public repository on a server that
/// has registered no App. The App's installation (see [`crate::App::owns`]) is the stronger proof
/// and is what a hosted server uses; this is what a self-host has.
#[derive(Debug)]
pub struct Open<T> {
    client: Client<T>,
}

impl<T: Transport> Open<T> {
    /// A proof that calls GitHub through `client`.
    pub const fn new(client: Client<T>) -> Self {
        Self { client }
    }

    /// The client, for a test to read what a fake was asked.
    pub const fn client(&self) -> &Client<T> {
        &self.client
    }

    /// Proves that whoever holds `challenge` can write to `repo`: the file [`Challenge::FILE`] on
    /// its default branch, read with no credential, has the challenge on a line. A repository that
    /// is private or has no such file is [`Unproven::Absent`]. A repository on another host is
    /// [`Unproven::Unlisted`], and nothing is sent.
    pub async fn prove(&self, repo: &Repo, challenge: &Challenge) -> Result<Proven, Unproven<T::Error>> {
        if repo.host != Host::GITHUB {
            return Err(Unproven::Unlisted { host: repo.host.clone() });
        }
        let path = &repo.path;
        let parts = ["repos", path.owner().as_str(), path.name().as_str(), "contents", ".well-known", "uses-ink"];
        let url = Host::API.url(&parts).map_err(Fail::Url)?;
        let request = Request::new(Method::Get, url).header("Accept", RAW).header("X-GitHub-Api-Version", VERSION);
        let response = self.client.send(request).await?;
        match response.status {
            404 => challenge.check(repo.clone(), None),
            status if !response.success() => Err(Unproven::Call(Fail::Status { status })),
            _ => challenge.check(repo.clone(), Some(response.text().unwrap_or(""))),
        }
    }
}
