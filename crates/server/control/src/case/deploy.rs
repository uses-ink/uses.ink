//! Deploys, the live pointer, rollback and resume.

use blob::Blob;
use build::BuildId;
use db::{Db, Statement};
use jiff::Timestamp;
use logging::Field as _;
use model::{Bytes, DeployId};
use sea_query::{Expr, ExprTrait, Func, JoinType, OnConflict, Order, Query};

use crate::deploy::{self, Cause, Deploy, Live, Published, Recorded, Resumed};
use crate::id::{Id, Site as SiteKind};
use crate::table::{self, Computed, Excluded};
use crate::{Control, Error, Event, Ports, Refusal};

impl<P: Ports> Control<P> {
    /// Records the deploy a build made and, unless publishing is paused, makes it live.
    ///
    /// Every blob the manifest names must be in the site's tenant already, and the manifest is
    /// stored as a blob too: its hash is the deploy's id, so the same site built twice is the
    /// same deploy and recording it again changes nothing. The pointer moves in one statement
    /// that records the previous deploy, so two builds finishing at once cannot lose the
    /// rollback target.
    pub async fn deploy(&self, new: deploy::New) -> Result<Recorded, Error<P>> {
        let site = self.active(new.site).await?;
        let tenant = Self::tenant(site.id)?;
        let bytes = self.stat(&tenant, &new.manifest).await?;
        self.store_room(&site, &new.manifest).await?;
        let blobs = self.ports.blob();
        let id = new.manifest.id();
        blobs.put(&tenant, id.hash(), [new.manifest.canonical()]).await?;
        let before = self.live(site.id).await?;
        let step = Move::new(site.id, id, new.at);
        let batch = vec![
            Event::DeployAdded { deploy: id }.fresh(site.id, new.at, id)?,
            step.record(new.build, new.manifest.entries().len() as u64, bytes)?,
            step.publish()?,
            step.history(Cause::Publish)?,
            Event::DeployPublished { deploy: id, previous: before.map(|live| live.deploy) }
                .moved(site.id, new.at, id)?,
        ];
        let outcomes = self.ports.db().batch(batch).await?;
        let moved = outcomes.get(2).is_some_and(|outcome| outcome.changes == 1);
        let published = match (moved, before) {
            (true, _) => Published::Live { previous: before.map(|live| live.deploy) },
            (false, Some(live)) if live.paused => Published::Held,
            (false, _) => Published::Unchanged,
        };
        logging::info!(
            name: logging::event::DEPLOY_PUBLISHED,
            deploy = logging::Digest(id.hash().as_bytes()).field(),
            outcome = match published {
                Published::Live { .. } => "live",
                Published::Held => "held",
                _ => "unchanged",
            },
            files = logging::Count::from(new.manifest.entries().len()).field(),
            bytes = logging::Size(bytes.count()).field(),
        );
        self.sweep(site.id).await?;
        Ok(Recorded { deploy: id, published })
    }

    /// The pointer `site` serves from, if a deploy is live.
    pub async fn live(&self, site: Id<SiteKind>) -> Result<Option<Live>, Error<P>> {
        let select = Query::select()
            .columns([table::Live::DeployId, table::Live::PreviousId, table::Live::Paused, table::Live::SetAt])
            .from(table::Live::Table)
            .and_where(Expr::col(table::Live::SiteId).eq(site.to_string()))
            .to_owned();
        match self.ports.db().first(select.into()).await? {
            Some(row) => Ok(Some(Live::try_from(&row)?)),
            None => Ok(None),
        }
    }

    /// The most recent `limit` deploys of `site`, newest first, each saying whether it is live.
    pub async fn deploys(&self, site: Id<SiteKind>, limit: u32) -> Result<Vec<Deploy>, Error<P>> {
        let select = Query::select()
            .columns([
                Self::deployed(table::Deploy::Id),
                Self::deployed(table::Deploy::CreatedAt),
                Self::deployed(table::Deploy::JobId),
                Self::deployed(table::Deploy::Files),
                Self::deployed(table::Deploy::Bytes),
            ])
            .expr_as(
                Func::coalesce([
                    Expr::col((table::Live::Table, table::Live::DeployId)).equals(Self::deployed(table::Deploy::Id)),
                    Expr::val(0_i64),
                ]),
                Computed::Live,
            )
            .from(table::Deploy::Table)
            .join(
                JoinType::LeftJoin,
                table::Live::Table,
                Expr::col((table::Live::Table, table::Live::SiteId)).equals(Self::deployed(table::Deploy::SiteId)),
            )
            .and_where(Expr::col(Self::deployed(table::Deploy::SiteId)).eq(site.to_string()))
            .order_by(Self::deployed(table::Deploy::CreatedAt), Order::Desc)
            .order_by(Self::deployed(table::Row::Rowid), Order::Desc)
            .limit(u64::from(limit))
            .to_owned();
        self.site(site).await?;
        let rows = self.ports.db().rows(select.into()).await?;
        Ok(rows.iter().map(Deploy::try_from).collect::<Result<_, _>>()?)
    }

    /// Makes an older deploy live again and pauses automatic publishing until [`Control::resume`]
    /// (architecture section 11): a build finishing later is recorded, and does not undo the
    /// rollback. The deploy replaced becomes the previous one, so rolling back twice returns.
    ///
    /// The pointer moves only if it still points where this call read it, so the event's
    /// `previous` is the deploy the pointer really left. A deploy that moved it in between makes
    /// this call read again.
    pub async fn rollback(&self, site: Id<SiteKind>, to: DeployId, at: Timestamp) -> Result<Live, Error<P>> {
        self.active(site).await?;
        loop {
            let before = self.live(site).await?.ok_or(Refusal::NotLive { site })?;
            let step = Move::new(site, to, at);
            let batch = vec![
                step.rollback(before.deploy),
                step.history(Cause::Rollback)?,
                Event::DeployRolledBack { deploy: to, previous: before.deploy }.moved(site, at, to)?,
            ];
            let outcomes = self.ports.db().batch(batch).await?;
            if outcomes.first().is_some_and(|outcome| outcome.changes == 1) {
                logging::info!(
                    name: logging::event::DEPLOY_ROLLED,
                    to = logging::Digest(to.hash().as_bytes()).field(),
                    from = logging::Digest(before.deploy.hash().as_bytes()).field(),
                );
                return self.live(site).await?.ok_or_else(|| Refusal::NotLive { site }.into());
            }
            let after = self.live(site).await?.ok_or(Refusal::NotLive { site })?;
            if after.deploy != before.deploy {
                continue;
            }
            if before.deploy == to {
                return Err(Refusal::AlreadyLive { site, deploy: to }.into());
            }
            return Err(Refusal::NoDeploy { site, deploy: to }.into());
        }
    }

    /// Lets builds publish again after a rollback paused them. It does not publish anything
    /// itself: the next deploy goes live.
    pub async fn resume(&self, site: Id<SiteKind>, at: Timestamp) -> Result<Resumed, Error<P>> {
        self.active(site).await?;
        let resumed = Event::SiteResumed;
        let resume = Query::update()
            .table(table::Live::Table)
            .value(table::Live::Paused, 0_i64)
            .and_where(Expr::col(table::Live::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Live::Paused).eq(1_i64))
            .to_owned();
        let batch = vec![resumed.paused(site, at)?, resume.into()];
        let outcomes = self.ports.db().batch(batch).await?;
        Ok(if outcomes.get(1).is_some_and(|outcome| outcome.changes == 1) {
            Resumed::Resumed
        } else {
            Resumed::NotPaused
        })
    }

    /// `column` of the deploy table, named with its table.
    fn deployed<C>(column: C) -> (table::Deploy, C) {
        (table::Deploy::Table, column)
    }

    /// The size of every blob the manifest names, or the refusal for the first one missing.
    pub(crate) async fn stat(&self, tenant: &blob::Tenant, manifest: &model::Manifest) -> Result<Bytes, Error<P>> {
        let mut bytes = Bytes(0);
        for entry in manifest.entries() {
            let size = self.ports.blob().stat(tenant, entry.blob()).await?;
            let size = size.ok_or_else(|| Refusal::MissingBlob { route: entry.route().clone(), blob: entry.blob() })?;
            bytes = Bytes(bytes.count().saturating_add(size.count()));
        }
        Ok(bytes)
    }
}

/// One move of a site's pointer to a deploy at a time, and the statements that make it and
/// record it.
#[derive(Debug)]
struct Move {
    site: String,
    deploy: String,
    at: i64,
}

impl Move {
    fn new(site: Id<SiteKind>, deploy: DeployId, at: Timestamp) -> Self {
        Self { site: site.to_string(), deploy: deploy.to_string(), at: at.as_millisecond() }
    }

    /// `column` of the live table, named with its table.
    fn live(column: table::Live) -> Expr {
        Expr::col((table::Live::Table, column))
    }

    /// `column` of the row an upsert proposed.
    fn proposed(column: table::Live) -> Expr {
        Expr::col((Excluded::Table, column))
    }

    /// The statement that records the deploy, once.
    fn record(&self, build: Option<BuildId>, files: u64, bytes: Bytes) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Deploy::Table)
            .columns([
                table::Deploy::SiteId,
                table::Deploy::Id,
                table::Deploy::CreatedAt,
                table::Deploy::JobId,
                table::Deploy::Files,
                table::Deploy::Bytes,
            ])
            .values([
                Expr::val(self.site.as_str()),
                Expr::val(self.deploy.as_str()),
                Expr::val(self.at),
                Expr::val(build.map(|build| build.to_string())),
                Expr::val(files.cast_signed()),
                Expr::val(bytes.count().cast_signed()),
            ])?
            .on_conflict(OnConflict::columns([table::Deploy::SiteId, table::Deploy::Id]).do_nothing().to_owned())
            .to_owned()
            .into())
    }

    /// The statement that points the site at the deploy, unless it is paused or already there,
    /// keeping what it replaced as the previous one.
    fn publish(&self) -> Result<Statement, sea_query::error::Error> {
        let replace = Self::live(table::Live::Paused)
            .eq(0_i64)
            .and(Self::live(table::Live::DeployId).ne(Self::proposed(table::Live::DeployId)));
        Ok(Query::insert()
            .into_table(table::Live::Table)
            .columns([
                table::Live::SiteId,
                table::Live::DeployId,
                table::Live::PreviousId,
                table::Live::Paused,
                table::Live::SetAt,
            ])
            .values([
                Expr::val(self.site.as_str()),
                Expr::val(self.deploy.as_str()),
                Expr::val(None::<String>),
                Expr::val(0_i64),
                Expr::val(self.at),
            ])?
            .on_conflict(
                OnConflict::column(table::Live::SiteId)
                    .value(table::Live::PreviousId, Self::live(table::Live::DeployId))
                    .value(table::Live::DeployId, Self::proposed(table::Live::DeployId))
                    .value(table::Live::SetAt, Self::proposed(table::Live::SetAt))
                    .action_and_where(replace)
                    .to_owned(),
            )
            .to_owned()
            .into())
    }

    /// The statement that points the site back at the deploy, when the site records it, it is
    /// not already live and the pointer is still at `from`, and pauses publishing.
    fn rollback(&self, from: DeployId) -> Statement {
        let held = Query::select()
            .column(table::Deploy::Id)
            .from(table::Deploy::Table)
            .and_where(Expr::col(table::Deploy::SiteId).eq(self.site.as_str()))
            .and_where(Expr::col(table::Deploy::Id).eq(self.deploy.as_str()))
            .to_owned();
        Query::update()
            .table(table::Live::Table)
            .value(table::Live::PreviousId, Expr::col(table::Live::DeployId))
            .value(table::Live::DeployId, self.deploy.as_str())
            .value(table::Live::Paused, 1_i64)
            .value(table::Live::SetAt, self.at)
            .and_where(Expr::col(table::Live::SiteId).eq(self.site.as_str()))
            .and_where(Expr::col(table::Live::DeployId).ne(self.deploy.as_str()))
            .and_where(Expr::col(table::Live::DeployId).eq(from.to_string()))
            .and_where(Expr::exists(held))
            .to_owned()
            .into()
    }

    /// The statement that records the move in the history, when the pointer is as the move set
    /// it: paused only if the cause is a rollback.
    fn history(&self, cause: Cause) -> Result<Statement, sea_query::error::Error> {
        let select = Query::select()
            .columns([table::Live::SiteId, table::Live::DeployId, table::Live::PreviousId])
            .expr(Expr::val(cause.name()))
            .column(table::Live::SetAt)
            .from(table::Live::Table)
            .and_where(Expr::col(table::Live::SiteId).eq(self.site.as_str()))
            .and_where(Expr::col(table::Live::DeployId).eq(self.deploy.as_str()))
            .and_where(Expr::col(table::Live::SetAt).eq(self.at))
            .and_where(Expr::col(table::Live::Paused).eq(i64::from(cause == Cause::Rollback)))
            .to_owned();
        let mut insert = Query::insert();
        insert
            .into_table(table::Publication::Table)
            .columns([
                table::Publication::SiteId,
                table::Publication::DeployId,
                table::Publication::PreviousId,
                table::Publication::Cause,
                table::Publication::At,
            ])
            .select_from(select)?;
        Ok(insert.into())
    }
}
