//! The removal table, as a migration after the abuse tables'.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Table};

use crate::table::Removal;
use crate::typed::Typed;

/// The migration that makes `removal`: a row for each removal that has begun, so a removal that
/// stops can be resumed and its receipt counts the whole of it. The counts of deploys, builds and
/// credentials are taken when it begins, before any row goes.
#[derive(Clone, Copy, Debug)]
pub struct Erasing;

impl Migration for Erasing {
    const NUMBER: u32 = 304;
    const NAME: &'static str = "removal";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Removal::Table)
                .col(ColumnDef::new(Removal::SiteId).text().not_null().primary_key())
                .col(Typed::text(Removal::AccountId))
                .col(Typed::text(Removal::Handle))
                .col(ColumnDef::new(Removal::Deploys).big_integer().not_null())
                .col(ColumnDef::new(Removal::Builds).big_integer().not_null())
                .col(ColumnDef::new(Removal::Credentials).big_integer().not_null())
                .col(ColumnDef::new(Removal::Blobs).big_integer().not_null())
                .col(ColumnDef::new(Removal::Bytes).big_integer().not_null())
                .into(),
        ]
    }
}
