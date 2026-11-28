//! The commit the watch last queued a build for, kept per site.

use db::Db;
use jiff::Timestamp;
use model::Oid;
use sea_query::{Expr, ExprTrait, OnConflict, Query};

use crate::fields::Fields;
use crate::id::{Id, Site as SiteKind};
use crate::table::{self, Excluded};
use crate::{Control, Error, Ports, Unreadable};

impl<P: Ports> Control<P> {
    /// The commit of `site`'s source the watch last queued a build for, if it has.
    pub async fn seen(&self, site: Id<SiteKind>) -> Result<Option<Oid>, Error<P>> {
        let select = Query::select()
            .column(table::Seen::Head)
            .from(table::Seen::Table)
            .and_where(Expr::col(table::Seen::SiteId).eq(site.to_string()))
            .to_owned();
        let Some(row) = self.ports.db().first(select.into()).await? else { return Ok(None) };
        let text = Fields(&row).text(table::Seen::Head)?;
        let head = Oid::parse(text).map_err(|source| Unreadable::Commit { source })?;
        Ok(Some(head))
    }

    /// Remembers that a build of `head` was queued for `site` at `at`. A site that is gone is
    /// refused by the row's reference to it.
    pub async fn see(&self, site: Id<SiteKind>, head: &Oid, at: Timestamp) -> Result<(), Error<P>> {
        self.active(site).await?;
        let upsert = Query::insert()
            .into_table(table::Seen::Table)
            .columns([table::Seen::SiteId, table::Seen::Head, table::Seen::At])
            .values([Expr::val(site.to_string()), Expr::val(head.as_str()), Expr::val(at.as_millisecond())])?
            .on_conflict(
                OnConflict::column(table::Seen::SiteId)
                    .value(table::Seen::Head, Expr::col((Excluded::Table, table::Seen::Head)))
                    .value(table::Seen::At, Expr::col((Excluded::Table, table::Seen::At)))
                    .to_owned(),
            )
            .to_owned();
        self.ports.db().run(upsert.into()).await?;
        Ok(())
    }
}
