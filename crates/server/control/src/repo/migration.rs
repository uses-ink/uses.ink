//! The repository claim table, as a migration after the proofs'.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Expr, ExprTrait, ForeignKey, IdenStatic, Index, Table};

use crate::table::{Credential, Named, Repo, Site};
use crate::typed::Typed;

/// The migration that makes `repo_claim`: a row for each repository a site asked to prove, with the
/// challenge it must commit and the sealed webhook secret that claim made. A row whose `owned` is
/// 1 is the repository's proof: the unique key over the repository and `owned` lets the database
/// keep one such row for a repository however many sites asked, because a null `owned` is distinct
/// from every other and the claims that are not proved all hold one (decision 0160).
#[derive(Clone, Copy, Debug)]
pub struct Bound;

impl Migration for Bound {
    const NUMBER: u32 = 311;
    const NAME: &'static str = "repo_claim";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Repo::Table)
                .col(Typed::text(Repo::Repo))
                .col(Typed::text(Repo::SiteId))
                .col(Typed::text(Repo::Challenge))
                .col(Typed::text(Repo::SecretId))
                .col(Typed::time(Repo::CreatedAt))
                // null until proved
                .col(ColumnDef::new(Repo::ProvenAt).big_integer())
                // 1 for the proved claim and null for the rest, so the unique key below holds one proof
                .col(ColumnDef::new(Repo::Owned).integer().check(Expr::col(Repo::Owned).eq(1_i64)))
                .primary_key(Index::create().col(Repo::Repo).col(Repo::SiteId))
                .foreign_key(ForeignKey::create().from(Repo::Table, Repo::SiteId).to(Site::Table, Site::Id))
                .foreign_key(
                    ForeignKey::create().from(Repo::Table, Repo::SecretId).to(Credential::Table, Credential::Id),
                )
                .into(),
            Index::create().name(Named::RepoSite.as_str()).table(Repo::Table).col(Repo::SiteId).into(),
            Index::create()
                .name(Named::RepoOwned.as_str())
                .table(Repo::Table)
                .col(Repo::Repo)
                .col(Repo::Owned)
                .unique()
                .into(),
        ]
    }
}
