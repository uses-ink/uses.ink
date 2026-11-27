//! What the API sends, as this client reads it, and what `--json` prints of it.
//!
//! The server's views (`edge_api::view`) only serialize, so the client has its own readers; the
//! JSON shapes are the ones the API documents.

use jiff::Timestamp;
use model::DeployId;
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
    pub(crate) state: String,
    /// When it was requested.
    pub(crate) requested: Timestamp,
    /// When it ended.
    pub(crate) finished: Option<Timestamp>,
    /// The deploy it made.
    pub(crate) deploy: Option<DeployId>,
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
