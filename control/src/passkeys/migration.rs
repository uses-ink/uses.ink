//! The tables of passkeys, as a migration after the keys'.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, ForeignKeyAction, IdenStatic, Index, Table};

use super::Flow;
use crate::migration::Typed;
use crate::table::{Account, Allowed, AuthConfig, Ceremony, Named, Passkey, PasskeySession, Session};

/// The migration that makes `passkey`, `passkey_session`, `ceremony` and `auth_config`. A passkey
/// row holds a public key and counters; the private key never leaves the authenticator.
#[derive(Clone, Copy, Debug)]
pub struct Entry;

impl Migration for Entry {
    const NUMBER: u32 = 202;
    const NAME: &'static str = "passkeys";

    fn statements() -> Vec<Statement> {
        [Self::passkeys(), Self::sessions(), Self::ceremonies(), Self::config()].concat()
    }
}

impl Entry {
    /// Credentials, by their own id. The authenticator's credential id is unique.
    fn passkeys() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Passkey::Table)
                .col(ColumnDef::new(Passkey::Id).text().not_null().primary_key())
                .col(Typed::text(Passkey::AccountId))
                .col(ColumnDef::new(Passkey::CredentialId).blob().not_null().unique_key())
                .col(ColumnDef::new(Passkey::PublicKey).blob().not_null())
                // 0 when the authenticator keeps no counter
                .col(ColumnDef::new(Passkey::SignCount).big_integer().not_null())
                .col(ColumnDef::new(Passkey::BackupEligible).boolean().not_null())
                .col(ColumnDef::new(Passkey::BackupState).boolean().not_null())
                // names from `Transport::NAMES`, comma separated; empty when none were reported
                .col(Typed::text(Passkey::Transports))
                .col(Typed::text(Passkey::Label))
                .col(Typed::time(Passkey::CreatedAt))
                // a date, `2026-11-27`, written when the day changes and no finer
                .col(ColumnDef::new(Passkey::LastUsedOn).text())
                .foreign_key(
                    ForeignKey::create().from(Passkey::Table, Passkey::AccountId).to(Account::Table, Account::Id),
                )
                .into(),
            Index::create().name(Named::PasskeyAccount.as_str()).table(Passkey::Table).col(Passkey::AccountId).into(),
        ]
    }

    /// Which sessions a passkey made. Both sides cascade: a removed passkey or a swept session
    /// takes its link with it.
    fn sessions() -> Vec<Statement> {
        vec![
            Table::create()
                .table(PasskeySession::Table)
                .col(ColumnDef::new(PasskeySession::SessionId).text().not_null().primary_key())
                .col(Typed::text(PasskeySession::PasskeyId))
                .foreign_key(
                    ForeignKey::create()
                        .from(PasskeySession::Table, PasskeySession::SessionId)
                        .to(Session::Table, Session::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(PasskeySession::Table, PasskeySession::PasskeyId)
                        .to(Passkey::Table, Passkey::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .into(),
        ]
    }

    /// Ceremonies in progress: the digest of the cookie that binds one to a browser, the state,
    /// and when it ends.
    fn ceremonies() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Ceremony::Table)
                .col(ColumnDef::new(Ceremony::Digest).blob().not_null().primary_key())
                .col(ColumnDef::new(Ceremony::Flow).text().not_null().check(Allowed::of(Ceremony::Flow, &Flow::NAMES)))
                .col(ColumnDef::new(Ceremony::AccountId).text())
                .col(ColumnDef::new(Ceremony::State).blob().not_null())
                .col(Typed::time(Ceremony::ExpiresAt))
                .into(),
            Index::create()
                .name(Named::CeremonyExpires.as_str())
                .table(Ceremony::Table)
                .col(Ceremony::ExpiresAt)
                .into(),
        ]
    }

    /// The relying party id of the first passkey, one row.
    fn config() -> Vec<Statement> {
        vec![
            Table::create()
                .table(AuthConfig::Table)
                .col(ColumnDef::new(AuthConfig::Slot).text().not_null().primary_key())
                .col(Typed::text(AuthConfig::RpId))
                .into(),
        ]
    }
}
