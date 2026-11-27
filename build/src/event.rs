//! The build log: one typed event stream (decision 0023).

use std::time::Duration;

use model::{AnyDecision, Bytes, DeployId, Diagnostic, Notice, Observed, Version};
use serde::{Deserialize, Serialize};

use crate::BuildId;

/// What set a build off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Trigger {
    /// Someone asked for it.
    Manual,
    /// A forge told us the source changed.
    Push,
    /// The poll found the source changed.
    Poll,
    /// Someone uploaded the source.
    Upload,
}

impl std::fmt::Display for Trigger {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Manual => "manual",
            Self::Push => "push",
            Self::Poll => "poll",
            Self::Upload => "upload",
        })
    }
}

/// How a build ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Status {
    /// A deploy was made.
    Succeeded,
    /// The site did not build, and the log says why. No deploy.
    Refused,
    /// The builder failed. No deploy.
    Failed,
}

/// One entry of a build's log. The same entries show in the terminal, the dashboard, storage
/// (as NDJSON in the blob store) and forge commit statuses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Event {
    /// The build began.
    Started {
        /// Which build.
        build: BuildId,
        /// What set it off.
        trigger: Trigger,
    },
    /// The source was read into a snapshot.
    SourceFetched {
        /// The version the snapshot is of.
        version: Version,
        /// How many files it holds.
        files: u64,
        /// How many bytes they hold.
        bytes: Bytes,
        /// What the source could and could not do.
        observed: Observed,
    },
    /// The engine recorded a decision.
    Decided(AnyDecision),
    /// The engine found a problem.
    Diagnosed(Diagnostic),
    /// The engine said something that is not a problem: a default chosen, a cache cold.
    Noticed(Notice),
    /// The site was rendered.
    Rendered {
        /// How many files the deploy serves.
        files: u64,
        /// How many bytes they hold.
        bytes: Bytes,
    },
    /// The deploy went live. The builder never says this: the control plane does, after it
    /// writes the pointer.
    Published {
        /// The deploy that is live now.
        deploy: DeployId,
        /// The one it replaced.
        previous: Option<DeployId>,
        /// How long until every reader sees it.
        visible_within: Duration,
    },
    /// The build stopped for this reason.
    Refused(Diagnostic),
    /// The build ended.
    Finished {
        /// How.
        status: Status,
        /// How long it took.
        wall: Duration,
    },
}

/// Where a build's events go.
pub trait Events {
    /// Takes the next event, in order.
    fn event(&mut self, event: Event);
}

impl Events for Vec<Event> {
    fn event(&mut self, event: Event) {
        self.push(event);
    }
}

impl trace::Kind for Trigger {
    fn kind(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Push => "push",
            Self::Poll => "poll",
            Self::Upload => "upload",
        }
    }
}

impl trace::Kind for Status {
    fn kind(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }
}
