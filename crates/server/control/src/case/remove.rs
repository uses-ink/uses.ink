//! Removing a site, bytes before rows (decision 0022).

use blob::{Blob, Deleted};
use db::{Db, Statement};
use jiff::Timestamp;
use model::Bytes;
use sea_query::{
    Asterisk, Expr, ExprTrait, Func, IdenStatic, IntoColumnRef, IntoTableRef, OnConflict, Query, SelectStatement,
    SubQueryStatement,
};

use crate::id::{Id, Site as SiteKind};
use crate::removal::Begun;
use crate::table::{self, Computed};
use crate::{Control, Error, Handle, Hold, HoldReason, Ports, Progress, Receipt, Removal, Site};

impl<P: Ports> Control<P> {
    /// Removes a site and everything of it, and returns what was removed: steps until one is
    /// done.
    pub async fn remove_site(&self, id: Id<SiteKind>, at: Timestamp) -> Result<Receipt, Error<P>> {
        loop {
            if let Removal::Done(receipt) = self.remove_step(id, at).await? {
                return Ok(receipt);
            }
        }
    }

    /// One bounded step of removing a site: at most [`Policy::removal`](crate::Policy) blobs
    /// are deleted, so no step is longer than the operator allows.
    ///
    /// The order is the point (decision 0022). The first step marks the site `removing`, drops
    /// its live pointer, so it serves nothing, and records the counts of what it had in a
    /// `removal` row. Every step deletes blobs and adds what it deleted to that row. The step
    /// that finds no more than its share left deletes them, then the rows, and leaves the handle
    /// held. A crash or a failing store between steps leaves the marked site and its row, which
    /// is a diagnosable state and never bytes without a row; calling again resumes, and the
    /// receipt counts the whole removal, not the part since the last start. A step that fails
    /// partway still records the blobs it deleted before the failure.
    pub async fn remove_step(&self, id: Id<SiteKind>, at: Timestamp) -> Result<Removal, Error<P>> {
        let site = self.site(id).await?;
        let tenant = Self::tenant(id)?;
        let begun = match self.begun(id).await? {
            Some(begun) => begun,
            None => self.begin(&site).await?,
        };
        let store = self.ports.blob();
        let share = usize::try_from(Ord::max(self.policy.removal, 1)).unwrap_or(usize::MAX);
        let listed = store.list(&tenant).await?;
        let last = listed.len() <= share;
        let (mut removed, mut bytes) = (0_u64, 0_u64);
        let mut failed = None;
        for entry in listed.into_iter().take(share) {
            match store.delete(&tenant, entry.hash).await {
                Ok(Deleted::Removed(held)) => {
                    removed += 1;
                    bytes = bytes.saturating_add(held.count());
                }
                Ok(_) => {}
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }
        if removed > 0 {
            self.ports.db().batch(vec![Begun::add(id, removed, bytes)]).await?;
        }
        if let Some(error) = failed {
            return Err(error.into());
        }
        if !last {
            let total = (begun.blobs.saturating_add(removed), begun.bytes.saturating_add(bytes));
            return Ok(Removal::More(Progress { site: id, blobs: total.0, bytes: Bytes(total.1) }));
        }
        self.erase(&site.handle, id, at).await?;
        Ok(Removal::Done(begun.receipt((removed, bytes), at)))
    }

    /// The removal of `site` that has begun, if one has.
    async fn begun(&self, site: Id<SiteKind>) -> Result<Option<Begun>, Error<P>> {
        let select = Query::select()
            .columns(Begun::COLUMNS)
            .from(table::Removal::Table)
            .and_where(Expr::col(table::Removal::SiteId).eq(site.to_string()))
            .to_owned();
        let row = self.ports.db().first(select.into()).await?;
        Ok(row.map(|row| Begun::read(&row)).transpose()?)
    }

    /// Begins a removal: takes the counts, marks the site and unpublishes it, in one batch.
    async fn begin(&self, site: &Site) -> Result<Begun, Error<P>> {
        let key = site.id.to_string();
        let db = self.ports.db();
        let counts = db.first(Self::counts(&key).into()).await?;
        let counts = counts.ok_or(db::ColumnError {
            column: Computed::Deploys.as_str().to_owned(),
            fault: db::ColumnFault::Missing,
        })?;
        let begun = Begun {
            site: site.id,
            account: site.account,
            handle: site.handle.clone(),
            deploys: counts.get(Computed::Deploys)?,
            builds: counts.get(Computed::Builds)?,
            credentials: counts.get(Computed::Credentials)?,
            blobs: 0,
            bytes: 0,
        };
        let mark = Query::update()
            .table(table::Site::Table)
            .value(table::Site::State, crate::site::State::Removing.name())
            .and_where(Expr::col(table::Site::Id).eq(key.clone()))
            .to_owned();
        let unpublish =
            Query::delete().from_table(table::Live::Table).and_where(Expr::col(table::Live::SiteId).eq(key)).to_owned();
        db.batch(vec![begun.insert()?, mark.into(), unpublish.into()]).await?;
        Ok(begun)
    }

    /// The number of rows of `table` that belong to `site`, as a subquery.
    fn count<T: IntoTableRef, C: IntoColumnRef + Copy>(table: T, column: C, site: &str) -> Expr {
        let rows = Query::select()
            .expr(Func::count(Expr::col(Asterisk)))
            .from(table)
            .and_where(Expr::col(column).eq(site))
            .to_owned();
        Expr::from(SubQueryStatement::SelectStatement(rows))
    }

    /// How many deploys, builds and credentials a site has, in one row.
    fn counts(site: &str) -> SelectStatement {
        Query::select()
            .expr_as(Self::count(table::Deploy::Table, table::Deploy::SiteId, site), Computed::Deploys)
            .expr_as(Self::count(table::Job::Table, table::Job::SiteId, site), Computed::Builds)
            .expr_as(Self::count(table::Credential::Table, table::Credential::SiteId, site), Computed::Credentials)
            .to_owned()
    }

    /// Deletes every row of a site, children first, and leaves its handle as a tombstone.
    async fn erase(&self, handle: &Handle, id: Id<SiteKind>, at: Timestamp) -> Result<(), Error<P>> {
        let key = id.to_string();
        let mut batch: Vec<Statement> = vec![
            Self::wipe(table::Event::Table, table::Event::SiteId, &key),
            Self::wipe(table::Bundle::Table, table::Bundle::SiteId, &key),
            Self::wipe(table::Pin::Table, table::Pin::SiteId, &key),
            Self::wipe(table::Seen::Table, table::Seen::SiteId, &key),
            Self::wipe(table::Removal::Table, table::Removal::SiteId, &key),
            Self::wipe(table::Preview::Table, table::Preview::SiteId, &key),
            Self::wipe(table::Doomed::Table, table::Doomed::SiteId, &key),
            Self::wipe(table::Proof::Table, table::Proof::SiteId, &key),
            Self::wipe(table::Domain::Table, table::Domain::SiteId, &key),
            Self::wipe(table::Claim::Table, table::Claim::SiteId, &key),
            Self::wipe(table::Suspension::Table, table::Suspension::SiteId, &key),
            Self::wipe(table::Live::Table, table::Live::SiteId, &key),
            Self::wipe(table::Publication::Table, table::Publication::SiteId, &key),
            Self::wipe(table::Job::Table, table::Job::SiteId, &key),
            Self::wipe(table::Credential::Table, table::Credential::SiteId, &key),
            Self::wipe(table::Deploy::Table, table::Deploy::SiteId, &key),
            Self::wipe(table::Source::Table, table::Source::SiteId, &key),
            Self::wipe(table::Site::Table, table::Site::Id, &key),
        ];
        let tombstone = Query::insert()
            .into_table(table::Tombstone::Table)
            .columns([table::Tombstone::Handle, table::Tombstone::SiteId, table::Tombstone::RemovedAt])
            .values([Expr::val(handle.as_str()), Expr::val(key), Expr::val(at.as_millisecond())])?
            .on_conflict(OnConflict::column(table::Tombstone::Handle).do_nothing().to_owned())
            .to_owned();
        batch.push(tombstone.into());
        batch.push(Hold::new(handle.clone(), id, HoldReason::Removed, at).lasting(self.policy.hold).insert()?);
        self.ports.db().batch(batch).await?;
        Ok(())
    }

    /// Deletes the rows of `table` whose `column` is `key`.
    fn wipe<T: IntoTableRef, C: IntoColumnRef>(table: T, column: C, key: &str) -> Statement {
        Query::delete().from_table(table).and_where(Expr::col(column).eq(key)).to_owned().into()
    }
}
