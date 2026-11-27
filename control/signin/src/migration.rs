//! The sign-in tables, as a migration after the domain's and the queue's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, IdenStatic, Index, Table};

use super::{Purpose, Sort};
use control_rows::typed::Typed;
use control_rows::table::{Account, Allowed, Challenge, Factor, Named, Session};

/// The migration that makes the tables of signing in: `factor`, `challenge` and `session`. Only
/// digests are stored: a token or a session secret is in no row, and no password exists.
#[derive(Clone, Copy, Debug)]
pub struct Entry;

impl Migration for Entry {
    const NUMBER: u32 = 200;
    const NAME: &'static str = "signin";

    fn statements() -> Vec<Statement> {
        [Self::factors(), Self::challenges(), Self::sessions()].concat()
    }
}

impl Entry {
    /// How an account proves itself.
    fn factors() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Factor::Table)
                .col(ColumnDef::new(Factor::Id).text().not_null().primary_key())
                .col(Typed::text(Factor::AccountId))
                .col(ColumnDef::new(Factor::Kind).text().not_null().check(Allowed::of(Factor::Kind, &Sort::NAMES)))
                // one account per address; other kinds of factor leave it null
                .col(ColumnDef::new(Factor::Address).text().unique_key())
                .col(Typed::time(Factor::CreatedAt))
                .foreign_key(
                    ForeignKey::create().from(Factor::Table, Factor::AccountId).to(Account::Table, Account::Id),
                )
                .into(),
        ]
    }

    /// One-time tokens. `slot` is set on the first-run link only and is unique, so it is made once.
    fn challenges() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Challenge::Table)
                .col(ColumnDef::new(Challenge::Digest).blob().not_null().primary_key())
                .col(
                    ColumnDef::new(Challenge::Purpose)
                        .text()
                        .not_null()
                        .check(Allowed::of(Challenge::Purpose, &Purpose::NAMES)),
                )
                .col(ColumnDef::new(Challenge::Address).text())
                .col(ColumnDef::new(Challenge::AccountId).text())
                .col(ColumnDef::new(Challenge::Slot).text().unique_key())
                .col(Typed::time(Challenge::CreatedAt))
                .col(Typed::time(Challenge::ExpiresAt))
                .col(ColumnDef::new(Challenge::UsedAt).big_integer())
                .into(),
            Index::create()
                .name(Named::ChallengeExpires.as_str())
                .table(Challenge::Table)
                .col(Challenge::ExpiresAt)
                .into(),
        ]
    }

    /// Sessions, by the digest of their secret.
    fn sessions() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Session::Table)
                .col(ColumnDef::new(Session::Id).text().not_null().primary_key())
                .col(ColumnDef::new(Session::Digest).blob().not_null().unique_key())
                .col(Typed::text(Session::AccountId))
                .col(Typed::time(Session::CreatedAt))
                .col(Typed::time(Session::SeenAt))
                .col(Typed::time(Session::ExpiresAt))
                .col(Typed::time(Session::LimitAt))
                .col(ColumnDef::new(Session::RevokedAt).big_integer())
                .foreign_key(
                    ForeignKey::create().from(Session::Table, Session::AccountId).to(Account::Table, Account::Id),
                )
                .into(),
            Index::create().name(Named::SessionAccount.as_str()).table(Session::Table).col(Session::AccountId).into(),
            Index::create().name(Named::SessionExpires.as_str()).table(Session::Table).col(Session::ExpiresAt).into(),
        ]
    }
}
