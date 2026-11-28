//! The doomed table, as a migration after the pin's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, Index, Table};

use crate::table::{Doomed, Site};
use crate::typed::Typed;

/// The migration that makes `doomed`: a row for each blob that no deploy and no preview names
/// any more, with its size and when it was queued. A blob is deleted only once its grace is over
/// and nothing names it again, so a build that skipped uploading a blob because it was held keeps
/// it until the build records its deploy (decision 0150).
#[derive(Clone, Copy, Debug)]
pub struct Condemned;

impl Migration for Condemned {
    const NUMBER: u32 = 309;
    const NAME: &'static str = "doomed";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Doomed::Table)
                .col(Typed::text(Doomed::SiteId))
                .col(Typed::text(Doomed::Blob))
                .col(ColumnDef::new(Doomed::Bytes).big_integer().not_null())
                .col(Typed::time(Doomed::Since))
                .primary_key(Index::create().col(Doomed::SiteId).col(Doomed::Blob))
                .foreign_key(ForeignKey::create().from(Doomed::Table, Doomed::SiteId).to(Site::Table, Site::Id))
                .into(),
        ]
    }
}
