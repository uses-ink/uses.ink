//! The GitLab authority: the repo challenge as proof, and commit statuses.

use serde::Deserialize;
use serde_json::json;
use url::Url;

use authority_forge::{
    Challenge, Client, Fail, Host, Method, Pinned, Proven, Repo, Request, Secret, State, Status, Transport, Unproven,
};

/// What `GET /projects/{id}` answers, as far as this reads it.
#[derive(Debug, Deserialize)]
struct Project {
    default_branch: Option<String>,
}

/// The GitLab authority over the instances an operator listed. GitLab's own is `gitlab.com`.
#[derive(Debug)]
pub struct Gitlab<T> {
    hosts: Vec<Host>,
    client: Client<T>,
}

impl<T: Transport> Gitlab<T> {
    /// An authority for the instances at `hosts`, calling through `client`.
    pub const fn new(hosts: Vec<Host>, client: Client<T>) -> Self {
        Self { hosts, client }
    }

    /// The authority for `gitlab.com` alone.
    pub fn hosted(client: Client<T>) -> Self {
        Self::new(vec![Host::GITLAB], client)
    }

    /// The client, for a test to read what a fake was asked.
    pub const fn client(&self) -> &Client<T> {
        &self.client
    }

    /// The URL under `/api/v4/projects/{path}`, where the path is one encoded segment, as
    /// GitLab takes it, followed by `rest`.
    fn project(repo: &Repo, rest: &[&str]) -> Result<Url, Fail<T::Error>> {
        let path = repo.path.to_string();
        let mut parts = vec!["api", "v4", "projects", path.as_str()];
        parts.extend(rest);
        repo.host.url(&parts).map_err(Fail::Url)
    }

    /// Proves that whoever holds `challenge` can write to `repo`: the file
    /// [`Challenge::FILE`] on the project's default branch, read with no credential, has the
    /// challenge on a line. A private project, or one with no such file, is
    /// [`Unproven::Absent`]. The instance must be listed.
    pub async fn prove(&self, repo: &Repo, challenge: &Challenge) -> Result<Proven, Unproven<T::Error>> {
        if !self.hosts.contains(&repo.host) {
            return Err(Unproven::Unlisted { host: repo.host.clone() });
        }
        let found = self.client.send(Request::new(Method::Get, Self::project(repo, &[])?)).await?;
        if found.status == 404 {
            return Err(Unproven::Absent);
        }
        if !found.success() {
            return Err(Unproven::Call(Fail::Status { status: found.status }));
        }
        let project: Project = found.json().map_err(Fail::Json)?;
        let Some(branch) = project.default_branch else { return Err(Unproven::Absent) };
        let mut url = Self::project(repo, &["repository", "files", Challenge::FILE, "raw"])?;
        url.query_pairs_mut().append_pair("ref", &branch);
        let file = self.client.send(Request::new(Method::Get, url)).await?;
        match file.status {
            404 => challenge.check(repo.clone(), None),
            status if !file.success() => Err(Unproven::Call(Fail::Status { status })),
            _ => challenge.check(repo.clone(), Some(file.text().unwrap_or(""))),
        }
    }

    /// Posts `status` to `at`, with `token` of a bot or an OAuth application. The token moves into
    /// the request and is dropped with it; this keeps no credential. GitLab's states are its own:
    /// a build that failed or could not run is `failed`.
    pub async fn status(&self, token: Secret, at: &Pinned, status: &Status) -> Result<(), Fail<T::Error>> {
        let url = Self::project(&at.repo, &["statuses", at.revision.as_str()])?;
        let state = match status.state {
            State::Pending => "pending",
            State::Success => "success",
            State::Failure | State::Error => "failed",
        };
        let body = json!({
            "state": state,
            "target_url": status.link.as_ref().map(Url::as_str),
            "description": status.note,
            "name": status.context,
        });
        let request = Request::new(Method::Post, url).bearer(token).json(&body).map_err(Fail::Json)?;
        let response = self.client.send(request).await?;
        if response.status != 201 {
            return Err(Fail::Status { status: response.status });
        }
        Ok(())
    }
}
