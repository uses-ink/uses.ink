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
use crate::table::{self, Computed};
use crate::{Control, Error, Ports, Receipt};

impl<P: Ports> Control<P> {
    /// Removes a site and everything of it, and returns what was removed.
    ///
    /// The order is the point. First the site is marked `removing` and its live pointer is
    /// dropped, so it serves nothing. Then every blob of its tenant is deleted. Only then are
    /// its rows deleted, and its handle left as a tombstone. A crash between the steps leaves
    /// the marked row, which is a diagnosable state and never bytes without a row; removing the
    /// site again finishes the job, and the receipt counts only what that run removed of blobs.
    pub async fn remove_site(&self, id: Id<SiteKind>, at: Timestamp) -> Result<Receipt, Error<P>> {
        let site = self.site(id).await?;
        let tenant = Self::tenant(id)?;
        let db = self.ports.db();
        let key = id.to_string();
        let counts = db.first(Self::counts(&key).into()).await?;
        let counts = counts.ok_or(db::ColumnError {
            column: Computed::Deploys.as_str().to_owned(),
            fault: db::ColumnFault::Missing,
        })?;
        let mark = Query::update()
            .table(table::Site::Table)
            .value(table::Site::State, crate::site::State::Removing.name())
            .and_where(Expr::col(table::Site::Id).eq(key.clone()))
            .to_owned();
        let unpublish =
            Query::delete().from_table(table::Live::Table).and_where(Expr::col(table::Live::SiteId).eq(key)).to_owned();
        db.batch(vec![mark.into(), unpublish.into()]).await?;
        let blobs = self.ports.blob();
        let (mut removed, mut bytes) = (0_u64, 0_u64);
        for listed in blobs.list(&tenant).await? {
            if let Deleted::Removed(size) = blobs.delete(&tenant, listed.hash).await? {
                removed += 1;
                bytes = bytes.saturating_add(size.count());
            }
        }
        self.erase(site.handle.as_str(), id, at).await?;
        Ok(Receipt {
            site: id,
            account: site.account,
            handle: site.handle,
            removed: at,
            deploys: counts.get(Computed::Deploys)?,
            blobs: removed,
            bytes: Bytes(bytes),
            builds: counts.get(Computed::Builds)?,
            credentials: counts.get(Computed::Credentials)?,
        })
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
    async fn erase(&self, handle: &str, id: Id<SiteKind>, at: Timestamp) -> Result<(), Error<P>> {
        let key = id.to_string();
        let mut batch: Vec<Statement> = vec![
            Self::wipe(table::Event::Table, table::Event::SiteId, &key),
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
            .values([Expr::val(handle), Expr::val(key), Expr::val(at.as_millisecond())])?
            .on_conflict(OnConflict::column(table::Tombstone::Handle).do_nothing().to_owned())
            .to_owned();
        batch.push(tombstone.into());
        self.ports.db().batch(batch).await?;
        Ok(())
    }

    /// Deletes the rows of `table` whose `column` is `key`.
    fn wipe<T: IntoTableRef, C: IntoColumnRef>(table: T, column: C, key: &str) -> Statement {
        Query::delete().from_table(table).and_where(Expr::col(column).eq(key)).to_owned().into()
    }
}
