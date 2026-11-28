//! The preview table, as a migration after the removal's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, Index, Table};

use crate::table::{Pin, Preview, Site};
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

/// The migration that lets a build job name the branch and commit it builds, for the previews the
/// watch queues: a row for each build of a branch and none for a build of the site's own source,
/// beside `bundle`, which does the same for a pushed tree. The index answers "was this commit of
/// this branch built already", so a restart queues nothing twice.
#[derive(Clone, Copy, Debug)]
pub struct Pinned;

impl Migration for Pinned {
    const NUMBER: u32 = 308;
    const NAME: &'static str = "pin";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Pin::Table)
                .col(ColumnDef::new(Pin::JobId).text().not_null().primary_key())
                .col(Typed::text(Pin::SiteId))
                .col(Typed::text(Pin::Branch))
                .col(Typed::text(Pin::Revision))
                .foreign_key(ForeignKey::create().from(Pin::Table, Pin::SiteId).to(Site::Table, Site::Id))
                .into(),
            Index::create()
                .name("pin_by_commit")
                .table(Pin::Table)
                .col(Pin::SiteId)
                .col(Pin::Branch)
                .col(Pin::Revision)
                .into(),
        ]
    }
}
