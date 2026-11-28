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

/// `factor`: how an account proves it is itself. Only the magic link exists today.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "factor"]
pub enum Factor {
    Table,
    Id,
    AccountId,
    Kind,
    Address,
    CreatedAt,
}

/// `challenge`: one-time tokens that have been issued, stored as digests.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "challenge"]
pub enum Challenge {
    Table,
    Digest,
    Purpose,
    Address,
    AccountId,
    Slot,
    CreatedAt,
    ExpiresAt,
    UsedAt,
}

/// `session`: signed-in sessions, stored as digests of their secret.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "session"]
pub enum Session {
    Table,
    Id,
    Digest,
    AccountId,
    CreatedAt,
    SeenAt,
    ExpiresAt,
    LimitAt,
    RevokedAt,
}

/// `passkey`: a WebAuthn credential of an account.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "passkey"]
pub enum Passkey {
    Table,
    Id,
    AccountId,
    CredentialId,
    PublicKey,
    SignCount,
    BackupEligible,
    BackupState,
    Transports,
    Label,
    CreatedAt,
    LastUsedOn,
}

/// `passkey_session`: which passkey signed a session in, so removing the passkey ends it.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "passkey_session"]
pub enum PasskeySession {
    Table,
    SessionId,
    PasskeyId,
}

/// `ceremony`: a registration or a sign-in that was started, by the digest of its cookie.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "ceremony"]
pub enum Ceremony {
    Table,
    Digest,
    Flow,
    AccountId,
    State,
    ExpiresAt,
}

/// `recovery_code`: the digest of one of an account's recovery codes, and when it was spent.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "recovery_code"]
pub enum RecoveryCode {
    Table,
    Id,
    Digest,
    AccountId,
    CreatedAt,
    UsedAt,
}

/// `auth_config`: the relying party id the first passkey was made under.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "auth_config"]
pub enum AuthConfig {
    Table,
    Slot,
    RpId,
}

/// `seen`: the commit of a site's source the watch queued a build for last.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "key"]
pub enum Key {
    Table,
    Id,
    AccountId,
    Kind,
    KeyType,
    PublicKey,
    Label,
    Rights,
    ScopeKind,
    SiteId,
    CreatedAt,
    ApprovedBy,
    ExpiresAt,
    LastUsedOn,
    RevokedAt,
    RevokedWhy,
}

#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "enrolment"]
pub enum Enrolment {
    Table,
    Id,
    UserCode,
    KeyId,
    KeyType,
    PublicKey,
    Label,
    Purpose,
    SiteId,
    Rights,
    Lifetime,
    State,
    CreatedAt,
    ExpiresAt,
    ApprovedBy,
    ApprovedAt,
}

#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "auth_event"]
pub enum AuthEvent {
    Table,
    Seq,
    AccountId,
    KeyId,
    At,
    Name,
    Body,
}

/// `bundle`: the uploaded tree a build of a push reads.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "bundle"]
pub enum Bundle {
    Table,
    JobId,
    SiteId,
    Tree,
}

#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "seen"]
pub enum Seen {
    Table,
    SiteId,
    Head,
    At,
}

/// `hold`: a handle that no site holds but that is not free: held for a time after its site left
/// it, or for good when its owner was verified.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "hold"]
pub enum Hold {
    Table,
    Handle,
    SiteId,
    Why,
    Until,
    At,
}

/// `abuse_report`: what a stranger told the operator about a site.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "abuse_report"]
pub enum Report {
    Table,
    Id,
    Handle,
    Kind,
    Detail,
    Contact,
    State,
    At,
}

/// `suspension`: a site the operator took down, and why.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "suspension"]
pub enum Suspension {
    Table,
    SiteId,
    Kind,
    By,
    ReportId,
    At,
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
    Count,
}

/// The names of the indexes.
#[derive(Clone, Copy, Debug, IdenStatic)]
pub enum Named {
    SiteAccount,
    JobSite,
    CredentialSite,
    EventSite,
    ChallengeExpires,
    SessionAccount,
    SessionExpires,
    KeyAccount,
    EnrolmentExpires,
    AuthEventAccount,
    PasskeyAccount,
    CeremonyExpires,
    RecoveryAccount,
}

/// The values a text column may hold, as a `CHECK`: the schema is built from the names the code
/// reads back, so the two cannot drift.
#[derive(Clone, Copy, Debug)]
pub struct Allowed;

impl Allowed {
    /// `column IN (names)`.
    pub fn of<C: IntoColumnRef, T>(column: C, names: &[(&'static str, T)]) -> Expr {
        Expr::col(column).is_in(names.iter().map(|(name, _)| *name))
    }
}
