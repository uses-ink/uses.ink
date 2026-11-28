//! The domain tables, as a migration after the preview's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, IdenStatic, Index, Table};

use crate::table::{Claim, Domain, Named, Site};
use crate::typed::Typed;

/// The migration that makes `domain` and `domain_claim`. A name has one row in `domain` (its
/// primary key), so of two sites that both prove it, the database lets one; a claim is a row per
/// site and name, so a name a stranger claimed first does not keep the owner from claiming it.
#[derive(Clone, Copy, Debug)]
pub struct Domained;

impl Migration for Domained {
    const NUMBER: u32 = 306;
    const NAME: &'static str = "domain";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Domain::Table)
                .col(ColumnDef::new(Domain::Name).text().not_null().primary_key())
                .col(Typed::text(Domain::SiteId))
                .col(Typed::time(Domain::ProvenAt))
                .foreign_key(ForeignKey::create().from(Domain::Table, Domain::SiteId).to(Site::Table, Site::Id))
                .into(),
            Index::create().name(Named::DomainSite.as_str()).table(Domain::Table).col(Domain::SiteId).into(),
            Table::create()
                .table(Claim::Table)
                .col(Typed::text(Claim::Name))
                .col(Typed::text(Claim::SiteId))
                .col(Typed::text(Claim::Token))
                .col(Typed::time(Claim::CreatedAt))
                .primary_key(Index::create().col(Claim::Name).col(Claim::SiteId))
                .foreign_key(ForeignKey::create().from(Claim::Table, Claim::SiteId).to(Site::Table, Site::Id))
                .into(),
            Index::create().name(Named::ClaimSite.as_str()).table(Claim::Table).col(Claim::SiteId).into(),
        ]
    }
}
