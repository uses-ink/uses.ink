//! The organization tables, as a migration after the domain's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, IdenStatic, Index, Table};

use super::Role;
use crate::table::{Account, Allowed, Member, Named, Org};
use crate::typed::Typed;

/// The migration that makes `org` and `member`. An organization is an account with a row in `org`;
/// a member is a person's account with a role the code names ([`Role`]), so no row holds a set of
/// rights and no custom role exists.
#[derive(Clone, Copy, Debug)]
pub struct Orged;

impl Migration for Orged {
    const NUMBER: u32 = 307;
    const NAME: &'static str = "org";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Org::Table)
                .col(ColumnDef::new(Org::Id).text().not_null().primary_key())
                .col(Typed::text(Org::Name))
                .col(Typed::time(Org::CreatedAt))
                .foreign_key(ForeignKey::create().from(Org::Table, Org::Id).to(Account::Table, Account::Id))
                .into(),
            Table::create()
                .table(Member::Table)
                .col(Typed::text(Member::OrgId))
                .col(Typed::text(Member::AccountId))
                .col(ColumnDef::new(Member::Role).text().not_null().check(Allowed::of(Member::Role, &Role::NAMES)))
                .col(Typed::time(Member::CreatedAt))
                .primary_key(Index::create().col(Member::OrgId).col(Member::AccountId))
                .foreign_key(ForeignKey::create().from(Member::Table, Member::OrgId).to(Org::Table, Org::Id))
                .foreign_key(
                    ForeignKey::create().from(Member::Table, Member::AccountId).to(Account::Table, Account::Id),
                )
                .into(),
            Index::create().name(Named::MemberAccount.as_str()).table(Member::Table).col(Member::AccountId).into(),
        ]
    }
}
