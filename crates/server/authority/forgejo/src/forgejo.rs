//! The Forgejo authority: the repo challenge as proof, and commit statuses.

use authority_forge::{
    Challenge, Client, Fail, Method, Pinned, Proven, Repo, Request, Secret, Status, Transport, Unproven,
};

use crate::Instances;

/// The Forgejo authority over the instances an operator listed.
#[derive(Debug)]
pub struct Forgejo<T> {
    instances: Instances,
    client: Client<T>,
}

impl<T: Transport> Forgejo<T> {
    /// An authority for `instances`, calling through `client`.
    pub const fn new(instances: Instances, client: Client<T>) -> Self {
        Self { instances, client }
    }

    /// The client, for a test to read what a fake was asked.
    pub const fn client(&self) -> &Client<T> {
        &self.client
    }

    /// The instances.
    pub const fn instances(&self) -> &Instances {
        &self.instances
    }

    /// Proves that whoever holds `challenge` can write to `repo`: the file
    /// [`Challenge::FILE`] on its default branch, read with no credential, has the challenge on a
    /// line. A repository that is private or has no such file is [`Unproven::Absent`]; the proof
    /// is for public repositories and needs no OAuth. The instance must be listed.
    pub async fn prove(&self, repo: &Repo, challenge: &Challenge) -> Result<Proven, Unproven<T::Error>> {
        if self.instances.find(&repo.host).is_none() {
            return Err(Unproven::Unlisted { host: repo.host.clone() });
        }
        let path = &repo.path;
        let parts =
            ["api", "v1", "repos", path.owner().as_str(), path.name().as_str(), "raw", ".well-known", "uses-ink"];
        let url = repo.host.url(&parts).map_err(Fail::Url)?;
        let response = self.client.send(Request::new(Method::Get, url)).await?;
        match response.status {
            404 => challenge.check(repo.clone(), None),
            status if !response.success() => Err(Unproven::Call(Fail::Status { status })),
            _ => challenge.check(repo.clone(), Some(response.text().unwrap_or(""))),
        }
    }

    /// Posts `status` to `at`, with `token` of the instance's OAuth application or a bot account.
    /// The token moves into the request and is dropped with it; this keeps no credential.
    pub async fn status(&self, token: Secret, at: &Pinned, status: &Status) -> Result<(), Fail<T::Error>> {
        let path = &at.repo.path;
        let parts =
            ["api", "v1", "repos", path.owner().as_str(), path.name().as_str(), "statuses", at.revision.as_str()];
        let url = at.repo.host.url(&parts).map_err(Fail::Url)?;
        let request = Request::new(Method::Post, url).bearer(token).json(&status.json()).map_err(Fail::Json)?;
        let response = self.client.send(request).await?;
        if response.status != 201 {
            return Err(Fail::Status { status: response.status });
        }
        Ok(())
    }
}
