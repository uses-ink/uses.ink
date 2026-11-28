//! Build jobs: a requested build, queued, running, then how it ended.

use build::{BuildId, Trigger};
use db::Row;
use jiff::Timestamp;
use model::{DeployId, Hash};
use queue::Dedupe;

use crate::Unreadable;
use crate::fields::Fields;
use crate::id::{Id, Site};
use crate::table;

/// Where a job is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// In the queue.
    Queued,
    /// A worker has it.
    Running,
    /// It made a deploy.
    Succeeded,
    /// The site does not build; the log says why.
    Refused,
    /// The builder failed and gave up.
    Failed,
}

impl State {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 5] = [
        ("queued", Self::Queued),
        ("running", Self::Running),
        ("succeeded", Self::Succeeded),
        ("refused", Self::Refused),
        ("failed", Self::Failed),
    ];

    /// The text this state is stored as, and the API writes.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, state)| *state == self).map_or("failed", |(name, _)| name)
    }

    /// Whether the build has ended.
    pub const fn ended(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.name())
    }
}

/// A trigger as the column stores it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stored(pub(crate) Trigger);

impl Stored {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Trigger); 4] =
        [("manual", Trigger::Manual), ("push", Trigger::Push), ("poll", Trigger::Poll), ("upload", Trigger::Upload)];

    /// The text this trigger is stored as.
    pub(crate) fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self.0).map_or("manual", |(name, _)| name)
    }
}

/// A build to request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct New {
    /// The build's id, drawn by the caller.
    pub id: BuildId,
    /// The site to build.
    pub site: Id<Site>,
    /// What set it off.
    pub trigger: Trigger,
    /// What makes the request once, such as a webhook delivery's id: a second request with the
    /// same key, while the first is still in the queue, is dropped.
    pub dedupe: Option<Dedupe>,
    /// What time it is.
    pub at: Timestamp,
}

/// What requesting a build did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Requested {
    /// The build is queued.
    Queued(BuildId),
    /// A request with the same key is in the queue already; nothing was added.
    Duplicate,
}

/// A requested build.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Job {
    /// The build's id.
    pub id: BuildId,
    /// The site.
    pub site: Id<Site>,
    /// What set it off.
    pub trigger: Trigger,
    /// Where it is.
    pub state: State,
    /// When it was requested.
    pub requested: Timestamp,
    /// When it ended, if it has.
    pub finished: Option<Timestamp>,
    /// The deploy it made, if it made one.
    pub deploy: Option<DeployId>,
    /// Its log, as a blob of the site's tenant.
    pub log: Option<Hash>,
    /// The branch and commit it builds, when it is a build of a preview.
    pub pin: Option<build::Pin>,
}

impl TryFrom<&Row> for Job {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            id: fields.text(table::Job::Id)?.parse().map_err(|source| Unreadable::Build { source })?,
            site: fields.id(table::Job::SiteId)?,
            trigger: fields.name(table::Job::Trigger, &Stored::NAMES)?,
            state: fields.name(table::Job::State, &State::NAMES)?,
            requested: fields.time(table::Job::RequestedAt)?,
            finished: fields.when(table::Job::FinishedAt)?,
            deploy: fields.maybe_deploy(table::Job::DeployId)?,
            log: fields.maybe_hash(table::Job::Log)?,
            pin: None,
        })
    }
}

/// What working one request off the queue came to.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Worked {
    /// The queue was empty.
    Idle,
    /// The site was built and the deploy recorded; `recorded.published` says if it is live.
    Built {
        /// The build.
        build: BuildId,
        /// What recording the deploy did.
        recorded: crate::deploy::Recorded,
    },
    /// The site does not build; the build's log says why.
    Refused {
        /// The build.
        build: BuildId,
    },
    /// A branch was built and its preview recorded; no deploy was made and nothing went live.
    Previewed {
        /// The build.
        build: BuildId,
        /// The preview's label.
        slug: crate::Slug,
    },
    /// The request could not be worked and was removed from the queue.
    Dropped(Dropped),
}

/// Why a queued request was dropped without a build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Dropped {
    /// Its body is no build id.
    Unnamed,
    /// No job has this build id: the request was sent and the process stopped before the job
    /// row was written.
    NoJob {
        /// The build.
        build: BuildId,
    },
    /// The job already ended: the request is a redelivery of one that was settled, and the job
    /// is as it was left.
    Settled {
        /// The build.
        build: BuildId,
    },
    /// The job's site is gone or being removed.
    NoSite {
        /// The build.
        build: BuildId,
        /// The site.
        site: Id<Site>,
    },
}

impl logging::Kind for &Dropped {
    fn kind(self) -> &'static str {
        match self {
            Dropped::Unnamed => "unnamed",
            Dropped::NoJob { .. } => "no-job",
            Dropped::Settled { .. } => "settled",
            _ => "no-site",
        }
    }
}
