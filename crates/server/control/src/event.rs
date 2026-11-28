//! The site's own log: one typed event per change, named in the past tense (conventions,
//! Grammar).

use std::fmt;

use build::BuildId;
use db::Statement;
use jiff::Timestamp;
use model::DeployId;
use sea_query::{Expr, ExprTrait, Query};
use serde::Serialize;

use crate::id::{Id, Site};
use crate::{Handle, table};

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
    /// The site's owner was verified, so its handle is never given to anyone else.
    HandleVerified {
        /// The handle.
        handle: Handle,
    },
    /// The operator suspended the site.
    SiteSuspended {
        /// Why.
        kind: crate::AbuseKind,
    },
    /// The operator lifted the suspension.
    SiteReinstated,
    /// A preview of a branch was made or replaced.
    PreviewPushed {
        /// Its label.
        slug: crate::Slug,
        /// The deploy it serves.
        deploy: DeployId,
    },
    /// A preview ended and was swept.
    PreviewExpired {
        /// Its label.
        slug: crate::Slug,
    },
    /// The site took another handle, and the old one is held.
    SiteRenamed {
        /// The handle it had.
        from: Handle,
        /// The handle it has.
        to: Handle,
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
            Self::HandleVerified { .. } => "handle.verified",
            Self::SiteRenamed { .. } => "site.renamed",
            Self::PreviewPushed { .. } => "preview.pushed",
            Self::PreviewExpired { .. } => "preview.expired",
            Self::SiteSuspended { .. } => "site.suspended",
            Self::SiteReinstated => "site.reinstated",
        }
    }
}

/// An event statement that could not be built.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unwritten {
    /// The event is not JSON.
    Json(serde_json::Error),
    /// The statement's columns and values do not match.
    Statement(sea_query::error::Error),
}

impl fmt::Display for Unwritten {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => out.write_str("an event could not be written as JSON"),
            Self::Statement(_) => out.write_str("an event statement could not be built"),
        }
    }
}

impl std::error::Error for Unwritten {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Statement(source) => Some(source),
        }
    }
}

impl From<serde_json::Error> for Unwritten {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(source)
    }
}

impl From<sea_query::error::Error> for Unwritten {
    fn from(source: sea_query::error::Error) -> Self {
        Self::Statement(source)
    }
}

impl Event {
    const COLUMNS: [table::Event; 4] = [table::Event::SiteId, table::Event::At, table::Event::Name, table::Event::Body];

    /// The four values of a row of the log.
    fn row(&self, site: Id<Site>, at: Timestamp) -> Result<[Expr; 4], Unwritten> {
        Ok([
            Expr::val(site.to_string()),
            Expr::val(at.as_millisecond()),
            Expr::val(self.name()),
            Expr::val(serde_json::to_string(self)?),
        ])
    }

    /// The statement that appends this event to `site`'s log.
    pub(crate) fn insert(&self, site: Id<Site>, at: Timestamp) -> Result<Statement, Unwritten> {
        let insert = Query::insert()
            .into_table(table::Event::Table)
            .columns(Self::COLUMNS)
            .values(self.row(site, at)?)?
            .to_owned();
        Ok(insert.into())
    }

    /// The statement that appends this event when `guard` holds.
    fn guarded(&self, site: Id<Site>, at: Timestamp, guard: Expr) -> Result<Statement, Unwritten> {
        let mut select = Query::select();
        select.exprs(self.row(site, at)?).and_where(guard);
        let mut insert = Query::insert();
        insert.into_table(table::Event::Table).columns(Self::COLUMNS).select_from(select)?;
        Ok(insert.into())
    }

    /// The statement that appends this event only if `site`'s pointer was set to `deploy` at
    /// `at`: that is, only if the statement before it moved the pointer.
    pub(crate) fn moved(&self, site: Id<Site>, at: Timestamp, deploy: DeployId) -> Result<Statement, Unwritten> {
        let moved = Query::select()
            .column(table::Live::SiteId)
            .from(table::Live::Table)
            .and_where(Expr::col(table::Live::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Live::DeployId).eq(deploy.to_string()))
            .and_where(Expr::col(table::Live::SetAt).eq(at.as_millisecond()))
            .to_owned();
        self.guarded(site, at, Expr::exists(moved))
    }

    /// The statement that appends this event only if `site` has not recorded `deploy` yet.
    pub(crate) fn fresh(&self, site: Id<Site>, at: Timestamp, deploy: DeployId) -> Result<Statement, Unwritten> {
        let known = Query::select()
            .column(table::Deploy::SiteId)
            .from(table::Deploy::Table)
            .and_where(Expr::col(table::Deploy::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Deploy::Id).eq(deploy.to_string()))
            .to_owned();
        self.guarded(site, at, Expr::not_exists(known))
    }

    /// The statement that appends this event only if `site` is paused.
    pub(crate) fn paused(&self, site: Id<Site>, at: Timestamp) -> Result<Statement, Unwritten> {
        let paused = Query::select()
            .column(table::Live::SiteId)
            .from(table::Live::Table)
            .and_where(Expr::col(table::Live::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Live::Paused).eq(1_i64))
            .to_owned();
        self.guarded(site, at, Expr::exists(paused))
    }
}
