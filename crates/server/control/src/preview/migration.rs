//! The preview table, as a migration after the removal's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Index, Table};

use crate::table::Preview;
use crate::typed::Typed;

/// The migration that makes `preview`: a row for each labelled preview of a site, with the
/// manifest it serves and when it ends. A preview is not a deploy and has no row in `deploy`, so
/// no listing of deploys, no live pointer and no rollback can reach it.
#[derive(Clone, Copy, Debug)]
pub struct Previewed;

impl Migration for Previewed {
    const NUMBER: u32 = 305;
    const NAME: &'static str = "preview";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Preview::Table)
                .col(Typed::text(Preview::SiteId))
                .col(Typed::text(Preview::Slug))
                .col(Typed::text(Preview::DeployId))
                .col(ColumnDef::new(Preview::Files).big_integer().not_null())
                .col(ColumnDef::new(Preview::Bytes).big_integer().not_null())
                .col(Typed::time(Preview::CreatedAt))
                .col(Typed::time(Preview::ExpiresAt))
                .primary_key(Index::create().col(Preview::SiteId).col(Preview::Slug))
                .into(),
        ]
    }
}
