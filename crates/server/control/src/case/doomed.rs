//! The grace a blob has before it is deleted (decision 0150). A deploy or a preview that goes
//! queues the blobs nothing else names; a queued blob is deleted only once its grace is over and
//! nothing names it again, so a build that skipped the upload of a blob because the store held
//! it can still record the deploy that names it.

use std::collections::BTreeSet;

use blob::{Blob, Deleted};
use db::{Db, Statement};
use jiff::Timestamp;
use model::{Bytes, Hash, Manifest};
use sea_query::{Expr, ExprTrait, Func, Order, Query};

use crate::doomed::Doomed;
use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::table::{self, Computed};
use crate::{Control, Error, Ports, Swept};

/// How many names one statement takes: under the smallest bound-parameter limit of a database.
const CHUNK: usize = 40;

impl<P: Ports> Control<P> {
    /// Queues the `goners` blobs of `site`, which no deploy and no preview names, at `at`. The
    /// caller changes the rows that named them, then calls [`Control::reap_site`], which deletes
    /// what is due: with no grace, all of them.
    pub(crate) async fn condemn(
        &self,
        site: Id<SiteKind>,
        goners: &BTreeSet<Hash>,
        at: Timestamp,
    ) -> Result<(), Error<P>> {
        let tenant = Self::tenant(site)?;
        let mut batch: Vec<Statement> = Vec::new();
        for blob in goners {
            if let Some(bytes) = self.ports.blob().stat(&tenant, *blob).await? {
                batch.push(Doomed { site, blob: *blob, bytes, since: at }.replace()?);
            }
        }
        if !batch.is_empty() {
            self.ports.db().batch(batch).await?;
        }
        Ok(())
    }

    /// The statements that take the blobs `manifest` names, and the manifest itself, off the queue
    /// of `site`, for the batch that records the deploy or the preview of it.
    pub(crate) fn revive(site: Id<SiteKind>, manifest: &Manifest) -> Vec<Statement> {
        let blobs = manifest.entries().iter().map(model::Served::blob).chain([manifest.id().hash()]);
        let names: Vec<String> = blobs.map(|blob| blob.to_string()).collect();
        names.chunks(CHUNK).map(|chunk| Doomed::clear(site, chunk.to_vec())).collect()
    }

    /// Deletes the queued blobs of `site` whose grace is over at `at` and that no deploy and no
    /// preview of it names, bytes before rows (decision 0022). A queued blob that is named again
    /// is taken off the queue and stays, and so is one of `keep`, which a deploy not recorded yet
    /// names.
    pub(crate) async fn reap_site(
        &self,
        site: Id<SiteKind>,
        at: Timestamp,
        keep: &BTreeSet<Hash>,
    ) -> Result<Swept, Error<P>> {
        let due = Query::select()
            .columns([table::Doomed::Blob])
            .from(table::Doomed::Table)
            .and_where(Expr::col(table::Doomed::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Doomed::Since).lte(self.cutoff(at)))
            .order_by(table::Doomed::Blob, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(due.into()).await?;
        let mut queued = Vec::new();
        for row in &rows {
            queued.push(Fields(row).hash(table::Doomed::Blob)?);
        }
        if queued.is_empty() {
            return Ok(Swept::default());
        }
        let tenant = Self::tenant(site)?;
        let mut held = keep.clone();
        for deploy in self.deploy_ids(site).await?.into_iter().chain(self.preview_ids(site, &[]).await?) {
            held.extend(self.reach(site, deploy).await?);
        }
        let mut swept = Swept::default();
        for blob in queued.iter().filter(|blob| !held.contains(blob)) {
            if let Deleted::Removed(size) = self.ports.blob().delete(&tenant, *blob).await? {
                swept.blobs += 1;
                swept.bytes = Bytes(swept.bytes.count().saturating_add(size.count()));
            }
        }
        self.dequeue(site, &queued, at).await?;
        Ok(swept)
    }

    /// Takes the `queued` blobs of `site` that were due at `at` off the queue. A blob queued again
    /// since the read keeps its newer row: only what was due is dropped.
    async fn dequeue(&self, site: Id<SiteKind>, queued: &[Hash], at: Timestamp) -> Result<(), Error<P>> {
        let names: Vec<String> = queued.iter().map(ToString::to_string).collect();
        let rows: Vec<Statement> = names
            .chunks(CHUNK)
            .map(|chunk| {
                Query::delete()
                    .from_table(table::Doomed::Table)
                    .and_where(Expr::col(table::Doomed::SiteId).eq(site.to_string()))
                    .and_where(Expr::col(table::Doomed::Blob).is_in(chunk.to_vec()))
                    .and_where(Expr::col(table::Doomed::Since).lte(self.cutoff(at)))
                    .to_owned()
                    .into()
            })
            .collect();
        self.ports.db().batch(rows).await?;
        Ok(())
    }

    /// Deletes the queued blobs of every site whose grace is over at `at`: what the periodic sweep
    /// of previews also does, so a blob queued by a site that does nothing more is still freed.
    pub async fn reap(&self, at: Timestamp) -> Result<Swept, Error<P>> {
        let due = Query::select()
            .distinct()
            .column(table::Doomed::SiteId)
            .from(table::Doomed::Table)
            .and_where(Expr::col(table::Doomed::Since).lte(self.cutoff(at)))
            .order_by(table::Doomed::SiteId, Order::Asc)
            .to_owned();
        let rows = self.ports.db().rows(due.into()).await?;
        let mut swept = Swept::default();
        for row in &rows {
            let freed = self.reap_site(Fields(row).id(table::Doomed::SiteId)?, at, &BTreeSet::new()).await?;
            swept.blobs += freed.blobs;
            swept.bytes = Bytes(swept.bytes.count().saturating_add(freed.bytes.count()));
        }
        Ok(swept)
    }

    /// The bytes of `site` that are queued: stored, and not counted against the account.
    pub(crate) async fn waiting(&self, site: Id<SiteKind>) -> Result<u64, Error<P>> {
        let sum = Query::select()
            .expr_as(
                Func::coalesce([Func::sum(Expr::col(table::Doomed::Bytes)).into(), Expr::val(0_i64)]),
                Computed::Count,
            )
            .from(table::Doomed::Table)
            .and_where(Expr::col(table::Doomed::SiteId).eq(site.to_string()))
            .to_owned();
        self.counted(sum).await
    }

    /// The latest `since` that is due at `at`: the grace before it.
    fn cutoff(&self, at: Timestamp) -> i64 {
        let grace = i64::try_from(self.policy.grace.as_millis()).unwrap_or(i64::MAX);
        at.as_millisecond().saturating_sub(grace)
    }
}
