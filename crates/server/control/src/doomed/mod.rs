//! A blob waiting out its grace before it is deleted (decision 0150).

mod migration;

use db::Statement;
use jiff::Timestamp;
use model::{Bytes, Hash};
use sea_query::{Expr, ExprTrait, OnConflict, Query};

use crate::id::{Id, Site};
use crate::table;

pub use migration::Condemned;

/// A blob queued for deletion: no deploy and no preview of the site named it when it was queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Doomed {
    pub(crate) site: Id<Site>,
    pub(crate) blob: Hash,
    pub(crate) bytes: Bytes,
    pub(crate) since: Timestamp,
}

impl Doomed {
    /// The statement that queues the blob, or queues it again from `since` if it was queued.
    pub(crate) fn replace(&self) -> Result<Statement, sea_query::error::Error> {
        Ok(Query::insert()
            .into_table(table::Doomed::Table)
            .columns([table::Doomed::SiteId, table::Doomed::Blob, table::Doomed::Bytes, table::Doomed::Since])
            .values([
                Expr::val(self.site.to_string()),
                Expr::val(self.blob.to_string()),
                Expr::val(self.bytes.count().cast_signed()),
                Expr::val(self.since.as_millisecond()),
            ])?
            .on_conflict(
                OnConflict::columns([table::Doomed::SiteId, table::Doomed::Blob])
                    .update_columns([table::Doomed::Bytes, table::Doomed::Since])
                    .to_owned(),
            )
            .to_owned()
            .into())
    }

    /// The statement that takes `blobs` of `site` off the queue: a deploy or a preview names them.
    pub(crate) fn clear(site: Id<Site>, blobs: Vec<String>) -> Statement {
        Query::delete()
            .from_table(table::Doomed::Table)
            .and_where(Expr::col(table::Doomed::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Doomed::Blob).is_in(blobs))
            .to_owned()
            .into()
    }
}
