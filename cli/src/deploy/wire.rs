//! What the API sends, as this client reads it, and what `--json` prints of it.
//!
//! The server's views (`edge_api::view`) only serialize, so the client has its own readers; the
//! JSON shapes are the ones the API documents.

use jiff::Timestamp;
use model::{DeployId, Hash};
use serde::{Deserialize, Serialize};

/// One deploy of the history.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Row {
    /// Its id.
    pub(crate) id: DeployId,
    /// When it was recorded.
    pub(crate) created: Timestamp,
    /// The build that made it.
    pub(crate) build: Option<String>,
    /// How many routes it serves.
    pub(crate) files: u64,
    /// How many bytes they hold.
    pub(crate) bytes: u64,
    /// Whether it is the one served now.
    pub(crate) live: bool,
}

/// Where a build is, as the API names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// In the queue.
    Queued,
    /// A worker has it.
    Running,
    /// It made a deploy.
    Succeeded,
    /// The site does not build.
    Refused,
    /// The builder failed.
    Failed,
}

impl Step {
    /// The word, as the API writes it.
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }

    /// Whether the build has ended.
    pub(crate) const fn ended(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

impl std::fmt::Display for Step {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.word())
    }
}

/// A list the API answers with.
#[derive(Debug, Deserialize)]
pub(crate) struct Items<T> {
    /// The rows, newest first.
    pub(crate) items: Vec<T>,
}

/// One build.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Built {
    /// Its id.
    pub(crate) id: String,
    /// What set it off.
    pub(crate) trigger: String,
    /// Where it is.
    pub(crate) state: Step,
    /// When it was requested.
    pub(crate) requested: Timestamp,
    /// When it ended.
    pub(crate) finished: Option<Timestamp>,
    /// The deploy it made.
    pub(crate) deploy: Option<DeployId>,
}

impl Built {
    /// A build that was just queued: it has an id and a step, and nothing else yet.
    pub(crate) fn named(id: &str, state: Step) -> Self {
        Self { id: id.to_owned(), trigger: "upload".to_owned(), state, requested: Timestamp::UNIX_EPOCH, finished: None, deploy: None }
    }
}

/// Where a site's content comes from.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum Origin {
    /// A git repository.
    Git {
        /// Where.
        url: String,
        /// The branch, tag or commit, when one is named.
        revision: Option<String>,
    },
    /// Folders sent with `ink push`.
    Upload,
    /// A folder on the builder's disk.
    Folder,
}

/// A site.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Site {
    /// Its id.
    pub(crate) id: String,
    /// Its handle.
    pub(crate) handle: String,
    /// The address it is served at.
    pub(crate) url: Option<String>,
    /// Whether it takes changes.
    pub(crate) state: String,
    /// When it was added.
    pub(crate) created: Timestamp,
    /// Where its content comes from.
    pub(crate) source: Origin,
}

/// What `GET /v1/sites/{site}` says.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Overview {
    /// The site.
    pub(crate) site: Site,
    /// What it serves, if anything.
    pub(crate) live: Option<Live>,
    /// The latest build.
    pub(crate) build: Option<Built>,
    /// The builds that are queued or running.
    pub(crate) pending: Vec<Built>,
}

/// What a tree sent to `POST /v1/sites/{site}/source` came to.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub(crate) enum Probed {
    /// Blobs are missing: send them, then the tree again.
    Missing {
        /// The blobs the site does not hold.
        missing: Vec<Hash>,
        /// How many bytes of files one request takes.
        part: u64,
    },
    /// The tree is stored and a build of it is queued.
    Queued {
        /// The build.
        build: String,
    },
    /// A build with this key is queued already.
    Duplicate,
}

/// What a site serves.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Live {
    /// The deploy.
    pub(crate) deploy: DeployId,
    /// What it replaced.
    pub(crate) previous: Option<DeployId>,
    /// Whether builds are held back after a rollback.
    pub(crate) paused: bool,
    /// Since when.
    pub(crate) since: Timestamp,
}

/// What a rollback or a resume left.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Moved {
    /// The pointer now.
    pub(crate) live: Option<Live>,
    /// For a resume: whether publishing had been paused.
    pub(crate) was_paused: Option<bool>,
}

/// The body of a rollback request.
#[derive(Debug, Serialize)]
pub(crate) struct Target {
    /// The deploy to make live.
    pub(crate) deploy: DeployId,
}

/// What `ink deploy list --json` prints.
#[derive(Debug, Serialize)]
pub(crate) struct Listed<'a> {
    pub(crate) schema: &'static str,
    pub(crate) site: String,
    pub(crate) total: usize,
    pub(crate) deploys: &'a [Row],
}

/// What `ink deploy show --json` prints.
#[derive(Debug, Serialize)]
pub(crate) struct Shown<'a> {
    pub(crate) schema: &'static str,
    pub(crate) deploy: &'a Row,
    pub(crate) build: Option<&'a Built>,
    /// The lines of the build's log, each one NDJSON as the API sent it.
    pub(crate) log: &'a [String],
}

/// What `ink deploy rollback` and `ink deploy resume --json` print.
#[derive(Debug, Serialize)]
pub(crate) struct Pointer<'a> {
    pub(crate) schema: &'static str,
    pub(crate) site: String,
    pub(crate) moved: &'a Moved,
}
