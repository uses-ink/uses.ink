//! The recovery code table, as a migration after the passkeys'.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, IdenStatic, Index, Table};

use crate::table::{Account, Named, RecoveryCode};
use crate::typed::Typed;

/// The migration that makes `recovery_code`. A row holds the digest of one code and when it was
/// spent; the code itself is in no row, and a new set deletes the old one.
#[derive(Clone, Copy, Debug)]
pub struct Entry;

impl Migration for Entry {
    const NUMBER: u32 = 203;
    const NAME: &'static str = "recovery";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(RecoveryCode::Table)
                .col(ColumnDef::new(RecoveryCode::Id).text().not_null().primary_key())
                .col(ColumnDef::new(RecoveryCode::Digest).blob().not_null().unique_key())
                .col(Typed::text(RecoveryCode::AccountId))
                .col(Typed::time(RecoveryCode::CreatedAt))
                .col(ColumnDef::new(RecoveryCode::UsedAt).big_integer())
                .foreign_key(
                    ForeignKey::create()
                        .from(RecoveryCode::Table, RecoveryCode::AccountId)
                        .to(Account::Table, Account::Id),
                )
                .into(),
            Index::create()
                .name(Named::RecoveryAccount.as_str())
                .table(RecoveryCode::Table)
                .col(RecoveryCode::AccountId)
                .into(),
        ]
    }
}
