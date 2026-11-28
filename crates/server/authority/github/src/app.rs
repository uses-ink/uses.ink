//! The App as the API sees it: ownership, installation tokens and commit statuses.

use build::Revision;
use jiff::Timestamp;
use serde::Deserialize;
use serde_json::json;

use authority_forge::{Client, Fail, Host, Method, Repo, Request, Secret, Segment, Status, Transport};

use crate::{AppId, Error, InstallationId, Jwt, Signer};

/// The media type GitHub asks for.
const ACCEPT: &str = "application/vnd.github+json";
/// The version of the REST API this is written against.
const VERSION: &str = "2022-11-28";

/// What `GET /repos/{owner}/{repo}/installation` answers, as far as this reads it.
#[derive(Debug, Deserialize)]
struct Installed {
    id: InstallationId,
    account: Account,
    suspended_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Account {
    login: String,
}

/// What `POST /app/installations/{id}/access_tokens` answers.
#[derive(Deserialize)]
struct Minted {
    token: String,
}

/// An installation and the one repository a token is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    /// The installation.
    pub installation: InstallationId,
    /// The repository.
    pub repo: Repo,
}

/// A commit a status is posted to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Whose token it takes.
    pub grant: Grant,
    /// The commit.
    pub revision: Revision,
}

/// That the App is installed on a repository, by the account that claimed it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Covered {
    /// The repository.
    pub repo: Repo,
    /// The installation that covers it.
    pub installation: InstallationId,
    /// The account the installation is on, which is the claimed one.
    pub account: Segment,
}

/// The GitHub App: its id, what signs for it and the client it speaks through.
///
/// It holds no token. A JWT is made for one call and an installation token for one use; both are
/// [`Secret`]s that move into a request and are dropped with it.
#[derive(Debug)]
pub struct App<S, T> {
    id: AppId,
    signer: S,
    client: Client<T>,
}

impl<S: Signer, T: Transport> App<S, T> {
    /// The App `id`, signing with `signer`, calling through `client`.
    pub const fn new(id: AppId, signer: S, client: Client<T>) -> Self {
        Self { id, signer, client }
    }

    /// The client, for a test to read what a fake was asked.
    pub const fn client(&self) -> &Client<T> {
        &self.client
    }

    fn request(method: Method, parts: &[&str]) -> Result<Request, Error<S::Error, T::Error>> {
        let url = Host::API.url(parts).map_err(|source| Error::Call(Fail::Url(source)))?;
        Ok(Request::new(method, url).header("Accept", ACCEPT).header("X-GitHub-Api-Version", VERSION))
    }

    /// Proves that the App is installed on `repo` by the account `claimed`: the installation of
    /// the repository is asked for with the App's JWT, and its account must be the repository's
    /// owner and the claimed account. A repository the App is not installed on is
    /// [`Error::NotInstalled`], and a suspended installation is [`Error::Suspended`].
    pub async fn owns(&self, claim: (&Repo, &Segment), now: Timestamp) -> Result<Covered, Error<S::Error, T::Error>> {
        let (repo, claimed) = claim;
        if repo.host != Host::GITHUB {
            return Err(Error::Unlisted { host: repo.host.clone() });
        }
        let jwt = Jwt::mint(&self.signer, self.id, now)?;
        let parts = ["repos", repo.path.owner().as_str(), repo.path.name().as_str(), "installation"];
        let response = self.client.send(Self::request(Method::Get, &parts)?.bearer(jwt)).await?;
        if response.status == 404 {
            return Err(Error::NotInstalled);
        }
        if !response.success() {
            return Err(Error::Call(Fail::Status { status: response.status }));
        }
        let installed: Installed = response.json().map_err(|source| Error::Call(Fail::Json(source)))?;
        if installed.suspended_at.is_some() {
            return Err(Error::Suspended);
        }
        let account: Segment = installed.account.login.parse().map_err(Error::Name)?;
        if &account != claimed || &account != repo.path.owner() {
            return Err(Error::Account { found: account });
        }
        Ok(Covered { repo: repo.clone(), installation: installed.id, account })
    }

    /// An installation token for one build, scoped to the one repository with read of contents and
    /// metadata and write of statuses, the permissions of HQ13. The caller holds it for the build
    /// and drops it after; this keeps nothing, so the next call exchanges again.
    pub async fn token(&self, grant: &Grant, now: Timestamp) -> Result<Secret, Error<S::Error, T::Error>> {
        let jwt = Jwt::mint(&self.signer, self.id, now)?;
        let id = grant.installation.to_string();
        let body = json!({
            "repositories": [grant.repo.path.name().as_str()],
            "permissions": { "contents": "read", "metadata": "read", "statuses": "write" },
        });
        let request = Self::request(Method::Post, &["app", "installations", &id, "access_tokens"])?
            .bearer(jwt)
            .json(&body)
            .map_err(|source| Error::Call(Fail::Json(source)))?;
        let response = self.client.send(request).await?;
        if response.status != 201 {
            return Err(Error::Call(Fail::Status { status: response.status }));
        }
        let minted: Minted = response.json().map_err(|source| Error::Call(Fail::Json(source)))?;
        Ok(Secret::new(minted.token.into_bytes()))
    }

    /// Posts `status` to the commit of `target`, with a token exchanged for this one call and
    /// dropped with the request.
    pub async fn status(
        &self,
        target: &Target,
        status: &Status,
        now: Timestamp,
    ) -> Result<(), Error<S::Error, T::Error>> {
        let token = self.token(&target.grant, now).await?;
        let path = &target.grant.repo.path;
        let parts = ["repos", path.owner().as_str(), path.name().as_str(), "statuses", target.revision.as_str()];
        let request = Self::request(Method::Post, &parts)?
            .bearer(token)
            .json(&status.json())
            .map_err(|source| Error::Call(Fail::Json(source)))?;
        let response = self.client.send(request).await?;
        if response.status != 201 {
            return Err(Error::Call(Fail::Status { status: response.status }));
        }
        Ok(())
    }
}
