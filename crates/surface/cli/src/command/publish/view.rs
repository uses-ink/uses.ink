//! What the publish commands print, as typed blocks: each is a value that renders into lines of
//! the terminal layer and serializes as the JSON of the same fields (`docs/cli.md` section 6).

use std::path::PathBuf;

use model::DeployId;
use serde::Serialize;

use crate::deploy::{Moment, Short, Size};
use crate::output::term::{Doc, Line, Record, Render, Role};
use build::BuildId;
use control::JobState;
use edge_api::view::{BuildView as Built, LiveView as Live, SourceView, Status as Overview};

/// How a site is doing, in one word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Health {
    /// Something is live and nothing is wrong.
    Ok,
    /// A build is queued or running.
    Building,
    /// Publishing is paused after a rollback: new deploys are recorded and not live.
    Paused,
    /// The latest build was refused or failed.
    Failing,
    /// Nothing was ever deployed.
    Empty,
}

impl Health {
    /// The word, and the role it is shown in.
    const fn shown(self) -> (&'static str, Role) {
        match self {
            Self::Ok => ("ok", Role::Good),
            Self::Building => ("building", Role::Warn),
            Self::Paused => ("paused", Role::Warn),
            Self::Failing => ("failing", Role::Bad),
            Self::Empty => ("empty", Role::Quiet),
        }
    }

    /// The health of a site from what its overview says.
    pub(crate) fn of(overview: &Overview) -> Self {
        let paused = overview.live.as_ref().is_some_and(|live| live.paused);
        let failed =
            overview.build.as_ref().is_some_and(|build| matches!(build.state, JobState::Refused | JobState::Failed));
        if paused {
            Self::Paused
        } else if !overview.pending.is_empty() {
            Self::Building
        } else if failed {
            Self::Failing
        } else if overview.live.is_some() {
            Self::Ok
        } else {
            Self::Empty
        }
    }
}

/// What `ink status --json` prints: the API's overview, and a word for how the site is doing.
#[derive(Debug, Serialize)]
pub(crate) struct Standing<'a> {
    pub(crate) schema: &'static str,
    #[serde(flatten)]
    pub(crate) overview: &'a Overview,
    pub(crate) health: Health,
    /// Where the domains are: the overview does not hold them, `ink domain list` does.
    pub(crate) domains: &'static str,
}

impl<'a> Standing<'a> {
    /// The schema of the document.
    pub(crate) const SCHEMA: &'static str = "uses.status/v1";

    /// The standing of the site `overview` describes.
    pub(crate) fn of(overview: &'a Overview) -> Self {
        Self {
            schema: Self::SCHEMA,
            overview,
            health: Health::of(overview),
            domains: "not listed here; `ink domain list` shows them",
        }
    }

    /// One build as a line: its id, where it is, what set it off and when.
    fn build(build: &Built) -> Line {
        let (role, word) = match build.state {
            JobState::Succeeded => (Role::Good, "succeeded"),
            JobState::Queued | JobState::Running => (Role::Warn, build.state.name()),
            JobState::Refused | JobState::Failed => (Role::Bad, build.state.name()),
        };
        Line::new()
            .with(Role::Id, build.id.to_string())
            .with(role, format!("  {word}"))
            .with(Role::Quiet, format!("  {}, requested {}", build.trigger, Moment(build.requested)))
    }

    /// The live deploy as a line.
    fn live(served: &Live) -> Line {
        let head = Line::new().with(Role::Good, Short(served.deploy).to_string());
        let head = head.with(Role::Quiet, format!("  since {}", Moment(served.since)));
        match served.previous {
            Some(previous) => head.with(Role::Quiet, format!(", after {}", Short(previous))),
            None => head,
        }
    }
}

impl Render for Standing<'_> {
    fn render(&self, into: &mut Doc) {
        let site = &self.overview.site;
        let address = site.url.as_ref().map_or_else(|| "no address".to_owned(), ToString::to_string);
        into.push(Line::new().strong(Role::Heading, site.handle.clone()).with(Role::Quiet, format!("  {address}")));
        into.blank();
        let (word, role) = self.health.shown();
        let source = match &site.source {
            SourceView::Upload => "upload: ink push sends the folder".to_owned(),
            SourceView::Folder => "a folder on the server".to_owned(),
            SourceView::Git { url, revision } => {
                format!("git {url}{}", revision.as_ref().map_or_else(String::new, |rev| format!(", {rev}")))
            }
        };
        let mut fields = Record::new(2)
            .field("site", Line::new().with(Role::Id, site.id.to_string()))
            .field("state", Line::plain(site.state.to_string()))
            .field("created", Line::plain(Moment(site.created).to_string()))
            .field("source", Line::plain(source))
            .field("health", Line::new().with(role, word));
        fields = match &self.overview.live {
            Some(live) => fields.field("live", Self::live(live)),
            None => fields.field("live", Line::new().with(Role::Quiet, "nothing yet")),
        };
        if self.overview.live.as_ref().is_some_and(|live| live.paused) {
            fields = fields.field(
                "paused",
                Line::new()
                    .with(Role::Warn, "new deploys are recorded, not live: ")
                    .with(Role::Literal, "ink deploy resume"),
            );
        }
        if let Some(build) = &self.overview.build {
            fields = fields.field("latest", Self::build(build));
        }
        for build in &self.overview.pending {
            fields = fields.field("pending", Self::build(build));
        }
        fields = fields.field("domains", Line::new().with(Role::Quiet, self.domains));
        fields.render(into);
    }
}

/// What `ink init` did.
#[derive(Debug, Serialize)]
pub(crate) struct Linked {
    pub(crate) schema: &'static str,
    pub(crate) site: String,
    pub(crate) handle: String,
    pub(crate) url: Option<String>,
    pub(crate) api: String,
    pub(crate) file: PathBuf,
    /// The command made the site, as opposed to linking one that was there.
    pub(crate) created: bool,
    /// The link was the one the folder had.
    pub(crate) unchanged: bool,
}

impl Linked {
    /// The schema of the document.
    pub(crate) const SCHEMA: &'static str = "uses.link/v1";
}

impl Render for Linked {
    fn render(&self, into: &mut Doc) {
        let what = match (self.created, self.unchanged) {
            (_, true) => "linked already; nothing was changed",
            (true, false) => "made, and linked to this folder",
            (false, false) => "linked to this folder",
        };
        let mut fields = Record::new(2)
            .field("site", Line::new().with(Role::Id, self.site.clone()))
            .field("handle", Line::plain(self.handle.clone()));
        if let Some(url) = &self.url {
            fields = fields.field("address", Line::plain(url.clone()));
        }
        fields = fields
            .field("api", Line::plain(self.api.clone()))
            .field("file", Line::plain(self.file.to_string_lossy()))
            .field("result", Line::new().with(Role::Good, what));
        fields.render(into);
    }
}

/// What `ink push --json` and `ink watch --json` print.
#[derive(Debug, Serialize)]
pub(crate) struct Pushed {
    pub(crate) schema: &'static str,
    pub(crate) site: String,
    /// The tree that was sent; none for `watch`.
    pub(crate) tree: Option<String>,
    pub(crate) build: BuildId,
    /// Where the build is: it ended when this is not queued or running.
    pub(crate) state: JobState,
    /// The deploy the build made, once it has.
    pub(crate) deploy: Option<DeployId>,
    /// Whether the deploy is the one served.
    pub(crate) live: Option<bool>,
    /// Files in the folder.
    pub(crate) files: Option<u64>,
    /// Files this run sent: the folder's, less what the site held already.
    pub(crate) sent: Option<u64>,
    /// Files the site held already.
    pub(crate) held: Option<u64>,
}

impl Pushed {
    /// The schema of `ink push`'s document.
    pub(crate) const PUSH: &'static str = "uses.push/v1";
    /// The schema of `ink watch`'s document.
    pub(crate) const WATCH: &'static str = "uses.watch/v1";
}

/// What `ink export --json` prints.
#[derive(Debug, Serialize)]
pub(crate) struct Exported {
    pub(crate) schema: &'static str,
    /// `site` for the files a deploy serves, `source` for what was pushed.
    pub(crate) what: &'static str,
    #[serde(serialize_with = "Exported::path")]
    pub(crate) dir: PathBuf,
    pub(crate) files: u64,
    pub(crate) bytes: u64,
}

impl Exported {
    /// The schema of the document.
    pub(crate) const SCHEMA: &'static str = "uses.export/v1";

    fn path<S: serde::Serializer>(path: &std::path::Path, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&path.to_string_lossy())
    }

    /// What was written, in words, for the closing line.
    pub(crate) fn said(&self) -> String {
        let noun = if self.files == 1 { "file" } else { "files" };
        format!("{} {noun}, {} of {} into {}", self.files, Size(self.bytes), self.what, self.dir.display())
    }
}
