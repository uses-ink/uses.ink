//! The site's own log: one typed event per change, named in the past tense (conventions,
//! Grammar).

use build::BuildId;
use db::Statement;
use jiff::Timestamp;
use model::DeployId;
use serde::Serialize;

use crate::id::{Id, Site};
use crate::Handle;

/// One entry of a site's log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Event {
    /// The site was made.
    SiteAdded {
        /// Its handle.
        handle: Handle,
    },
    /// A deploy was recorded.
    DeployAdded {
        /// The deploy.
        deploy: DeployId,
    },
    /// The pointer moved to a deploy that was built.
    DeployPublished {
        /// The deploy.
        deploy: DeployId,
        /// What it replaced.
        previous: Option<DeployId>,
    },
    /// The pointer moved back to an older deploy, and publishing paused.
    DeployRolledBack {
        /// The deploy.
        deploy: DeployId,
        /// What it replaced.
        previous: DeployId,
    },
    /// Publishing resumed.
    SiteResumed,
    /// A build was requested.
    BuildRequested {
        /// The build.
        build: BuildId,
    },
}

impl Event {
    /// The name the log stores: noun, a dot, the past-tense verb.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::SiteAdded { .. } => "site.added",
            Self::DeployAdded { .. } => "deploy.added",
            Self::DeployPublished { .. } => "deploy.published",
            Self::DeployRolledBack { .. } => "deploy.rolled-back",
            Self::SiteResumed => "site.resumed",
            Self::BuildRequested { .. } => "build.requested",
        }
    }
}

impl Event {
    const INSERT: &'static str = "INSERT INTO event (site_id, at, name, body) VALUES (?, ?, ?, ?)";
    const MOVED: &'static str = "INSERT INTO event (site_id, at, name, body) SELECT ?, ?, ?, ? \
        WHERE EXISTS (SELECT 1 FROM site_live WHERE site_id = ? AND deploy_id = ? AND set_at = ?)";
    const FRESH: &'static str = "INSERT INTO event (site_id, at, name, body) SELECT ?, ?, ?, ? \
        WHERE NOT EXISTS (SELECT 1 FROM deploy WHERE site_id = ? AND id = ?)";

    /// The statement that appends this event to `site`'s log.
    pub(crate) fn insert(&self, site: Id<Site>, at: Timestamp) -> Result<Statement, serde_json::Error> {
        Ok(Statement::prepare(Self::INSERT).bind(site.to_string()).bind(at.as_millisecond()).bind(self.name()).bind(serde_json::to_string(self)?))
    }

    /// The statement that appends this event only if `site`'s pointer was set to `deploy` at
    /// `at`: that is, only if the statement before it moved the pointer.
    pub(crate) fn moved(&self, site: Id<Site>, at: Timestamp, deploy: DeployId) -> Result<Statement, serde_json::Error> {
        Ok(Statement::prepare(Self::MOVED)
            .bind(site.to_string())
            .bind(at.as_millisecond())
            .bind(self.name())
            .bind(serde_json::to_string(self)?)
            .bind(site.to_string())
            .bind(deploy.to_string())
            .bind(at.as_millisecond()))
    }

    /// The statement that appends this event only if `site` has not recorded `deploy` yet.
    pub(crate) fn fresh(&self, site: Id<Site>, at: Timestamp, deploy: DeployId) -> Result<Statement, serde_json::Error> {
        Ok(Statement::prepare(Self::FRESH)
            .bind(site.to_string())
            .bind(at.as_millisecond())
            .bind(self.name())
            .bind(serde_json::to_string(self)?)
            .bind(site.to_string())
            .bind(deploy.to_string()))
    }
}
