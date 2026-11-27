//! The control plane's tables and columns as typed identifiers. Every statement names a table or
//! a column through one of these, so a misspelling is a compile error and the schema has one
//! definition.

use sea_query::{Expr, ExprTrait, IdenStatic, IntoColumnRef};

/// `account`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "account"]
pub enum Account {
    Table,
    Id,
    CreatedAt,
}

/// `site`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "site"]
pub enum Site {
    Table,
    Id,
    AccountId,
    Handle,
    State,
    CreatedAt,
}

/// `source`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "source"]
pub enum Source {
    Table,
    SiteId,
    Kind,
    Locator,
    Revision,
}

/// `tombstone`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "tombstone"]
pub enum Tombstone {
    Table,
    Handle,
    SiteId,
    RemovedAt,
}

/// `deploy`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "deploy"]
pub enum Deploy {
    Table,
    SiteId,
    Id,
    CreatedAt,
    JobId,
    Files,
    Bytes,
}

/// `site_live`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "site_live"]
pub enum Live {
    Table,
    SiteId,
    DeployId,
    PreviousId,
    Paused,
    SetAt,
}

/// `publication`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "publication"]
pub enum Publication {
    Table,
    Seq,
    SiteId,
    DeployId,
    PreviousId,
    Cause,
    At,
}

/// `job`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "job"]
pub enum Job {
    Table,
    Id,
    SiteId,
    Trigger,
    State,
    RequestedAt,
    FinishedAt,
    DeployId,
    Log,
}

/// `credential`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "credential"]
pub enum Credential {
    Table,
    Id,
    SiteId,
    Kind,
    Sealed,
    CreatedAt,
}

/// `event`.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "event"]
pub enum Event {
    Table,
    Seq,
    SiteId,
    At,
    Name,
    Body,
}

/// SQLite's row number, which orders rows that tie on a column.
#[derive(Clone, Copy, Debug, IdenStatic)]
pub enum Row {
    Rowid,
}

/// The row an upsert proposed, which `ON CONFLICT` can read.
#[derive(Clone, Copy, Debug, IdenStatic)]
pub enum Excluded {
    Table,
}

/// Columns a select makes up.
#[derive(Clone, Copy, Debug, IdenStatic)]
pub enum Computed {
    Live,
    Deploys,
    Builds,
    Credentials,
}

/// The names of the indexes.
#[derive(Clone, Copy, Debug, IdenStatic)]
pub enum Named {
    SiteAccount,
    JobSite,
    CredentialSite,
    EventSite,
}

/// The values a text column may hold, as a `CHECK`: the schema is built from the names the code
/// reads back, so the two cannot drift.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Allowed;

impl Allowed {
    /// `column IN (names)`.
    pub(crate) fn of<C: IntoColumnRef, T>(column: C, names: &[(&'static str, T)]) -> Expr {
        Expr::col(column).is_in(names.iter().map(|(name, _)| *name))
    }
}
