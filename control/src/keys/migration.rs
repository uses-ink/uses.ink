//! The tables of keys, enrolments and the credential audit, as a migration after the sign-in's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, ForeignKey, ForeignKeyAction, IdenStatic, Index, Table};

use super::enrol::State;
use super::key::{Kind, Why};
use crate::typed::Typed;
use crate::table::{Account, Allowed, AuthEvent, Enrolment, Key, Named, Site};
use sig::KeyType;

/// The migration that makes `key`, `enrolment` and `auth_event`. A key row holds a public key
/// and nothing that signs in; no password exists. Nothing here stores a nonce: the nonce store
/// is memory plus the boot barrier (decision 0068, open choice 7).
#[derive(Clone, Copy, Debug)]
pub struct Entry;

impl Migration for Entry {
    const NUMBER: u32 = 201;
    const NAME: &'static str = "keys";

    fn statements() -> Vec<Statement> {
        [Self::keys(), Self::enrolments(), Self::audit()].concat()
    }
}

impl Entry {
    /// Device and deploy keys, by their derived id.
    fn keys() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Key::Table)
                .col(ColumnDef::new(Key::Id).text().not_null().primary_key())
                .col(Typed::text(Key::AccountId))
                .col(ColumnDef::new(Key::Kind).text().not_null().check(Allowed::of(Key::Kind, &Kind::NAMES)))
                .col(ColumnDef::new(Key::KeyType).text().not_null().check(Allowed::of(Key::KeyType, &KeyType::NAMES)))
                .col(ColumnDef::new(Key::PublicKey).blob().not_null())
                .col(Typed::text(Key::Label))
                .col(Typed::text(Key::Rights))
                .col(
                    ColumnDef::new(Key::ScopeKind)
                        .text()
                        .not_null()
                        .check(Allowed::of(Key::ScopeKind, &[("account", ()), ("site", ())])),
                )
                .col(ColumnDef::new(Key::SiteId).text())
                .col(Typed::time(Key::CreatedAt))
                .col(Typed::text(Key::ApprovedBy))
                .col(Typed::time(Key::ExpiresAt))
                // a date, `2026-11-27`, written when the day changes and no finer
                .col(ColumnDef::new(Key::LastUsedOn).text())
                .col(ColumnDef::new(Key::RevokedAt).big_integer())
                .col(ColumnDef::new(Key::RevokedWhy).text().check(Allowed::of(Key::RevokedWhy, &Why::NAMES)))
                .foreign_key(ForeignKey::create().from(Key::Table, Key::AccountId).to(Account::Table, Account::Id))
                .foreign_key(
                    ForeignKey::create()
                        .from(Key::Table, Key::SiteId)
                        .to(Site::Table, Site::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .into(),
            Index::create().name(Named::KeyAccount.as_str()).table(Key::Table).col(Key::AccountId).into(),
        ]
    }

    /// Keys waiting for a person.
    fn enrolments() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Enrolment::Table)
                .col(ColumnDef::new(Enrolment::Id).text().not_null().primary_key())
                .col(ColumnDef::new(Enrolment::UserCode).text().not_null().unique_key())
                .col(ColumnDef::new(Enrolment::KeyId).text().not_null().unique_key())
                .col(
                    ColumnDef::new(Enrolment::KeyType)
                        .text()
                        .not_null()
                        .check(Allowed::of(Enrolment::KeyType, &KeyType::NAMES)),
                )
                .col(ColumnDef::new(Enrolment::PublicKey).blob().not_null())
                .col(Typed::text(Enrolment::Label))
                .col(
                    ColumnDef::new(Enrolment::Purpose)
                        .text()
                        .not_null()
                        .check(Allowed::of(Enrolment::Purpose, &Kind::NAMES)),
                )
                .col(ColumnDef::new(Enrolment::SiteId).text())
                .col(Typed::text(Enrolment::Rights))
                // seconds the key will live once approved
                .col(ColumnDef::new(Enrolment::Lifetime).big_integer().not_null())
                .col(
                    ColumnDef::new(Enrolment::State)
                        .text()
                        .not_null()
                        .check(Allowed::of(Enrolment::State, &State::NAMES)),
                )
                .col(Typed::time(Enrolment::CreatedAt))
                .col(Typed::time(Enrolment::ExpiresAt))
                .col(ColumnDef::new(Enrolment::ApprovedBy).text())
                .col(ColumnDef::new(Enrolment::ApprovedAt).big_integer())
                .into(),
            Index::create()
                .name(Named::EnrolmentExpires.as_str())
                .table(Enrolment::Table)
                .col(Enrolment::ExpiresAt)
                .into(),
        ]
    }

    /// The lifecycle of credentials, one typed event a row.
    fn audit() -> Vec<Statement> {
        vec![
            Table::create()
                .table(AuthEvent::Table)
                .col(ColumnDef::new(AuthEvent::Seq).integer().not_null().auto_increment().primary_key())
                .col(Typed::text(AuthEvent::AccountId))
                .col(Typed::text(AuthEvent::KeyId))
                .col(Typed::time(AuthEvent::At))
                .col(Typed::text(AuthEvent::Name))
                .col(Typed::text(AuthEvent::Body))
                .into(),
            Index::create()
                .name(Named::AuthEventAccount.as_str())
                .table(AuthEvent::Table)
                .col(AuthEvent::AccountId)
                .into(),
        ]
    }
}
