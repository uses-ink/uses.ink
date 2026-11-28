//! `ink repo claim|prove|rotate|list|release`: a site's source repository (decision 0160), over the
//! API's repository routes. `claim` prints the challenge to commit and the webhook secret to give
//! the forge, once; `prove` has the server read the forge; a proved repository builds the site when
//! its forge says it moved.

use authority_forge::Repo as Named;
use clap::{Args, Subcommand};
use edge_api::RepoStep;
use edge_api::view::{RepoGiven, RepoReleased, RepoView};
use serde::Serialize;

use crate::deploy::{Moment, Remote};
use crate::output::Io;
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::term::{Doc, Empty, Line, Listed, Record, Render, Role};

/// `ink repo <claim|prove|rotate|list|release>`: a group with no behaviour of its own.
#[derive(Debug, Args)]
pub struct Repo {
    /// What to do with the repository.
    #[command(subcommand)]
    pub command: Steps,
    /// The site's folder, from `-C`, whose link names the site.
    #[arg(skip)]
    pub dir: std::path::PathBuf,
}

/// What `ink repo` does.
#[derive(Debug, Subcommand)]
pub enum Steps {
    /// Ask to prove the repository: print the challenge to commit and the webhook secret.
    Claim(Which),
    /// Read the forge for the challenge, and let pushes to the repository build the site.
    Prove(Which),
    /// Put a new webhook secret in place of the old one, and print it.
    Rotate(Which),
    /// The repositories the site has claimed, and where each stands.
    List(Remote),
    /// Let go of the repository: pushes stop building the site and the secret is deleted.
    Release(Which),
}

/// A command about one repository.
#[derive(Debug, Args)]
pub struct Which {
    /// `host/owner/name`, such as `codeberg.org/me/blog`; the site's own git source when left out.
    pub repo: Option<Named>,
    /// Which site.
    #[command(flatten)]
    pub remote: Remote,
}

/// What `ink repo claim` and `rotate --json` print: the claim and the secret, once.
#[derive(Debug, Serialize)]
struct GivenOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    given: &'a RepoGiven,
}

/// What `ink repo prove --json` prints.
#[derive(Debug, Serialize)]
struct RepoOut<'a> {
    schema: &'static str,
    #[serde(flatten)]
    repo: &'a RepoView,
}

/// What `ink repo list --json` prints.
#[derive(Debug, Serialize)]
struct ReposOut<'a> {
    schema: &'static str,
    site: String,
    repos: &'a [RepoView],
}

/// What `ink repo release --json` prints.
#[derive(Debug, Serialize)]
struct LetGo<'a> {
    schema: &'static str,
    #[serde(flatten)]
    gone: &'a RepoReleased,
}

/// One repository as a record: where it stands and, until it is proved, what to do.
#[derive(Debug)]
struct Standing<'a>(&'a RepoView);

impl Render for Standing<'_> {
    fn render(&self, into: &mut Doc) {
        let repo = self.0;
        let (role, word) =
            if repo.proven.is_some() { (Role::Good, "building on push") } else { (Role::Warn, "waiting for proof") };
        let mut record = Record::new(0)
            .field("repo", Line::new().with(Role::Id, repo.repo.clone()))
            .field("state", Line::new().with(role, word));
        if let Some(at) = repo.proven {
            record = record.field("since", Line::plain(Moment(at).to_string()));
        } else {
            record = record
                .field("commit", Line::plain(format!("{} to {} on the default branch", repo.challenge, repo.file)));
        }
        if let (Some(forge), Some(hook)) = (&repo.forge, &repo.hook) {
            record =
                record.field("webhook", Line::plain(format!("{forge}: POST {hook} on the API host, JSON push events")));
        }
        record.render(into);
    }
}

/// A claim with its secret, shown once.
#[derive(Debug)]
struct Given<'a>(&'a RepoGiven);

impl Render for Given<'_> {
    fn render(&self, into: &mut Doc) {
        Standing(&self.0.claim).render(into);
        Record::new(0)
            .field("secret", Line::new().with(Role::Id, self.0.secret.reveal()))
            .field("note", Line::plain("shown once: give it to the forge as the webhook secret, or rotate it"))
            .render(into);
    }
}

/// What the list of repositories says when the site has claimed none.
const EMPTY: Empty = Empty {
    said: "No repositories claimed.",
    next: Some(("ink repo claim", "Ask to build the site when its repository moves")),
};

/// The repositories as a table.
#[derive(Debug)]
struct Listing<'a>(&'a [RepoView]);

impl Render for Listing<'_> {
    fn render(&self, into: &mut Doc) {
        let mut table = Listed::new(EMPTY, &["repo", "state", "since or challenge"]);
        for repo in self.0 {
            let (role, word) = if repo.proven.is_some() {
                (Role::Good, "building on push")
            } else {
                (Role::Warn, "waiting for proof")
            };
            let last = repo.proven.map_or_else(|| repo.challenge.clone(), |at| Moment(at).to_string());
            table = table.row(vec![
                Line::new().with(Role::Id, repo.repo.clone()),
                Line::new().with(role, word),
                Line::plain(last),
            ]);
        }
        table.render(into);
    }
}

/// A repository that was let go.
#[derive(Debug)]
struct Gone<'a>(&'a RepoReleased);

impl Render for Gone<'_> {
    fn render(&self, into: &mut Doc) {
        Record::new(0)
            .field("repo", Line::new().with(Role::Id, self.0.repo.clone()))
            .field("state", Line::new().with(Role::Good, "released"))
            .render(into);
    }
}

impl Steps {
    /// Runs the command.
    pub(crate) fn run(self, dir: &std::path::Path, io: &mut Io<'_>) -> Result<Exit, Error> {
        let fail = |source| Error::Deploy { source };
        match self {
            Self::Claim(which) => {
                let calls = which.remote.folder(dir)?;
                let given = calls.block(calls.api.repo_given(RepoStep::Claim, which.repo.as_ref())).map_err(fail)?;
                io.result(&Given(&given))?;
                io.json(&GivenOut { schema: "uses.repo-claim/v1", given: &given })?;
            }
            Self::Rotate(which) => {
                let calls = which.remote.folder(dir)?;
                let given = calls.block(calls.api.repo_given(RepoStep::Rotate, which.repo.as_ref())).map_err(fail)?;
                io.result(&Given(&given))?;
                io.json(&GivenOut { schema: "uses.repo-claim/v1", given: &given })?;
            }
            Self::Prove(which) => {
                let calls = which.remote.folder(dir)?;
                let repo = calls.block(calls.api.repo_prove(which.repo.as_ref())).map_err(fail)?;
                io.result(&Standing(&repo))?;
                io.json(&RepoOut { schema: "uses.repo/v1", repo: &repo })?;
            }
            Self::List(remote) => {
                let calls = remote.folder(dir)?;
                let repos = calls.block(calls.api.repos()).map_err(fail)?;
                io.result(&Listing(&repos))?;
                io.json(&ReposOut { schema: "uses.repos/v1", site: calls.api.site.to_string(), repos: &repos })?;
            }
            Self::Release(which) => {
                let calls = which.remote.folder(dir)?;
                let gone = calls.block(calls.api.repo_release(which.repo.as_ref())).map_err(fail)?;
                io.result(&Gone(&gone))?;
                io.json(&LetGo { schema: "uses.repo-released/v1", gone: &gone })?;
            }
        }
        Ok(Exit::Done)
    }
}
