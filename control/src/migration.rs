//! The control plane's migration: the rows of architecture section 11, one set for D1 and SQLite.
//! Times are unix milliseconds. Ids are 32 lower-case hex digits. A deploy's id is the hash of
//! its manifest. Nothing here is derived: the Site, decisions and rendered output are not stored.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Expr, ExprTrait, ForeignKey, IdenStatic, Index, IntoIden, Table};

use crate::credential::Use;
use crate::deploy::Cause;
use crate::job::{State as JobState, Stored as Trigger};
use crate::site::{State as SiteState, Stored as SourceKind};
use crate::table::{
    Account, Allowed, Credential, Deploy, Event, Job, Live, Named, Publication, Seen, Site, Source, Tombstone,
};

/// The two kinds of column most of the schema is made of.
pub(crate) struct Typed;

impl Typed {
    /// A text column that cannot be null.
    pub(crate) fn text<C: IntoIden>(column: C) -> ColumnDef {
        ColumnDef::new(column).text().not_null().to_owned()
    }

    /// A column of unix milliseconds that cannot be null.
    pub(crate) fn time<C: IntoIden>(column: C) -> ColumnDef {
        ColumnDef::new(column).big_integer().not_null().to_owned()
    }
}

/// The migration that makes the control plane's tables.
#[derive(Clone, Copy, Debug)]
pub struct Domain;

impl Migration for Domain {
    const NUMBER: u32 = 1;
    const NAME: &'static str = "domain";

    fn statements() -> Vec<Statement> {
        [Self::identity(), Self::history(), Self::pointer(), Self::moves(), Self::builds(), Self::log()].concat()
    }
}

/// The migration that remembers, per site, the commit the watch last queued a build for, so a
/// restart queues nothing for a source that has not moved.
#[derive(Clone, Copy, Debug)]
pub struct Remembered;

impl Migration for Remembered {
    const NUMBER: u32 = 2;
    const NAME: &'static str = "seen";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Seen::Table)
                .col(ColumnDef::new(Seen::SiteId).text().not_null().primary_key())
                .col(Typed::text(Seen::Head))
                .col(Typed::time(Seen::At))
                .foreign_key(ForeignKey::create().from(Seen::Table, Seen::SiteId).to(Site::Table, Site::Id))
                .into(),
        ]
    }
}

impl Domain {
    /// The accounts, their sites and what a site is made from.
    fn identity() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Account::Table)
                .col(ColumnDef::new(Account::Id).text().not_null().primary_key())
                .col(Typed::time(Account::CreatedAt))
                .into(),
            Table::create()
                .table(Site::Table)
                .col(ColumnDef::new(Site::Id).text().not_null().primary_key())
                .col(Typed::text(Site::AccountId))
                .col(ColumnDef::new(Site::Handle).text().not_null().unique_key())
                // `removing` is the marker a deletion leaves first, so a crash is diagnosable (decision 0022)
                .col(ColumnDef::new(Site::State).text().not_null().check(Allowed::of(Site::State, &SiteState::NAMES)))
                .col(Typed::time(Site::CreatedAt))
                .foreign_key(ForeignKey::create().from(Site::Table, Site::AccountId).to(Account::Table, Account::Id))
                .into(),
            Index::create().name(Named::SiteAccount.as_str()).table(Site::Table).col(Site::AccountId).into(),
            Table::create()
                .table(Source::Table)
                .col(ColumnDef::new(Source::SiteId).text().not_null().primary_key())
                .col(
                    ColumnDef::new(Source::Kind).text().not_null().check(Allowed::of(Source::Kind, &SourceKind::KINDS)),
                )
                .col(Typed::text(Source::Locator))
                .col(ColumnDef::new(Source::Revision).text())
                .foreign_key(ForeignKey::create().from(Source::Table, Source::SiteId).to(Site::Table, Site::Id))
                .into(),
            // A removed site's handle stays unclaimable
            Table::create()
                .table(Tombstone::Table)
                .col(ColumnDef::new(Tombstone::Handle).text().not_null().primary_key())
                .col(Typed::text(Tombstone::SiteId))
                .col(Typed::time(Tombstone::RemovedAt))
                .into(),
            // The manifest, which lists every route and blob of the deploy, is the blob whose hash is
            // `id`: the rows say only that the deploy exists and what it cost.
        ]
    }

    /// What a deploy is: its row.
    fn history() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Deploy::Table)
                .col(Typed::text(Deploy::SiteId))
                .col(Typed::text(Deploy::Id))
                .col(Typed::time(Deploy::CreatedAt))
                .col(ColumnDef::new(Deploy::JobId).text())
                .col(ColumnDef::new(Deploy::Files).big_integer().not_null())
                .col(ColumnDef::new(Deploy::Bytes).big_integer().not_null())
                .primary_key(Index::create().col(Deploy::SiteId).col(Deploy::Id))
                .foreign_key(ForeignKey::create().from(Deploy::Table, Deploy::SiteId).to(Site::Table, Site::Id))
                .into(),
        ]
    }

    /// The pointer a site serves from.
    fn pointer() -> Vec<Statement> {
        vec![
            // The one pointer a site serves from. `previous_id` is where a rollback goes back to;
            // `paused` is set by a rollback and cleared by `resume`, and while it is set a new deploy
            // is recorded and not published.
            Table::create()
                .table(Live::Table)
                .col(ColumnDef::new(Live::SiteId).text().not_null().primary_key())
                .col(Typed::text(Live::DeployId))
                .col(ColumnDef::new(Live::PreviousId).text())
                .col(
                    ColumnDef::new(Live::Paused)
                        .integer()
                        .not_null()
                        .default(Expr::value(0_i64))
                        .check(Expr::col(Live::Paused).is_in([0_i64, 1])),
                )
                .col(Typed::time(Live::SetAt))
                .foreign_key(ForeignKey::create().from(Live::Table, Live::SiteId).to(Site::Table, Site::Id))
                .foreign_key(
                    ForeignKey::create()
                        .from(Live::Table, (Live::SiteId, Live::DeployId))
                        .to(Deploy::Table, (Deploy::SiteId, Deploy::Id)),
                )
                .into(),
        ]
    }

    /// Every move of the pointer.
    fn moves() -> Vec<Statement> {
        vec![
            // Every move of the pointer, newest last: the history rollback reads.
            Table::create()
                .table(Publication::Table)
                .col(ColumnDef::new(Publication::Seq).integer().not_null().auto_increment().primary_key())
                .col(Typed::text(Publication::SiteId))
                .col(Typed::text(Publication::DeployId))
                .col(ColumnDef::new(Publication::PreviousId).text())
                .col(
                    ColumnDef::new(Publication::Cause)
                        .text()
                        .not_null()
                        .check(Allowed::of(Publication::Cause, &Cause::NAMES)),
                )
                .col(Typed::time(Publication::At))
                .foreign_key(
                    ForeignKey::create().from(Publication::Table, Publication::SiteId).to(Site::Table, Site::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(Publication::Table, (Publication::SiteId, Publication::DeployId))
                        .to(Deploy::Table, (Deploy::SiteId, Deploy::Id)),
                )
                .into(),
        ]
    }

    /// The build jobs and the credentials a site's source needs.
    fn builds() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Job::Table)
                .col(ColumnDef::new(Job::Id).text().not_null().primary_key())
                .col(Typed::text(Job::SiteId))
                .col(ColumnDef::new(Job::Trigger).text().not_null().check(Allowed::of(Job::Trigger, &Trigger::NAMES)))
                .col(ColumnDef::new(Job::State).text().not_null().check(Allowed::of(Job::State, &JobState::NAMES)))
                .col(Typed::time(Job::RequestedAt))
                .col(ColumnDef::new(Job::FinishedAt).big_integer())
                .col(ColumnDef::new(Job::DeployId).text())
                // the hash of the build's NDJSON log, a blob of the site's tenant
                .col(ColumnDef::new(Job::Log).text())
                .foreign_key(ForeignKey::create().from(Job::Table, Job::SiteId).to(Site::Table, Site::Id))
                .into(),
            Index::create()
                .name(Named::JobSite.as_str())
                .table(Job::Table)
                .col(Job::SiteId)
                .col(Job::RequestedAt)
                .into(),
            // Credentials are sealed with the row's id as associated data (decision 0031); the sealed
            // bytes are never read back except to open them.
            Table::create()
                .table(Credential::Table)
                .col(ColumnDef::new(Credential::Id).text().not_null().primary_key())
                .col(Typed::text(Credential::SiteId))
                .col(
                    ColumnDef::new(Credential::Kind)
                        .text()
                        .not_null()
                        .check(Allowed::of(Credential::Kind, &Use::NAMES)),
                )
                .col(ColumnDef::new(Credential::Sealed).blob().not_null())
                .col(Typed::time(Credential::CreatedAt))
                .foreign_key(ForeignKey::create().from(Credential::Table, Credential::SiteId).to(Site::Table, Site::Id))
                .into(),
            Index::create()
                .name(Named::CredentialSite.as_str())
                .table(Credential::Table)
                .col(Credential::SiteId)
                .into(),
            // The site's append-only log: one past-tense name and a JSON body per change.
        ]
    }

    /// The site's own log.
    fn log() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Event::Table)
                .col(ColumnDef::new(Event::Seq).integer().not_null().auto_increment().primary_key())
                .col(Typed::text(Event::SiteId))
                .col(Typed::time(Event::At))
                .col(Typed::text(Event::Name))
                .col(Typed::text(Event::Body))
                .into(),
        ]
    }
}

#[cfg(test)]
mod test {
    use db::Renderer;

    use super::*;

    fn rendered() -> Vec<String> {
        Domain::statements().iter().map(|statement| Renderer.render(statement).unwrap().text().to_owned()).collect()
    }

    #[test]
    fn checks_are_written_from_the_names_the_code_reads() {
        let text = rendered().join("\n");
        assert!(text.contains(r#"CHECK ("state" IN ('active', 'removing'))"#), "{text}");
        assert!(text.contains(r#"CHECK ("cause" IN ('publish', 'rollback'))"#), "{text}");
        assert!(text.contains(r#"CHECK ("paused" IN (0, 1))"#), "{text}");
    }

    #[test]
    fn keys_and_references_are_declared() {
        let text = rendered().join("\n");
        assert!(text.contains(r#"PRIMARY KEY ("site_id", "id")"#), "{text}");
        assert!(
            text.contains(r#"FOREIGN KEY ("site_id", "deploy_id") REFERENCES "deploy" ("site_id", "id")"#),
            "{text}"
        );
        assert!(text.contains("AUTOINCREMENT"), "{text}");
    }
}
