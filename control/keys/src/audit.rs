//! The lifecycle of credentials as typed events (`docs/auth.md` section 13).
//!
//! Two kinds of record are kept apart on purpose. These **events** are lifecycle changes: a key
//! approved, revoked, found expired, a replay seen. Each names the credential and the account and
//! never a secret. "Last used" is **a date on the key's own row**, written when the day changes;
//! there is no per-request row, no address, no user agent and no path. Refusals are counted by
//! the server's metrics, never logged per attempt.

use db::Row;
use jiff::Timestamp;
use serde::Serialize;
use sig::KeyId;

use super::key::{Kind, Why};
use super::right::{Rights, Scope};
use crate::unreadable::Unreadable;
use crate::fields::Fields;
use crate::id::{Account as AccountKind, Id};
use crate::table;

/// One lifecycle event of a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Audit {
    /// A person approved an enrolment and the key went live.
    KeyApproved {
        /// The key.
        key: KeyId,
        /// What kind of key.
        kind: Kind,
        /// What it may do.
        rights: Rights,
        /// Where.
        scope: Scope,
    },
    /// A key was revoked.
    KeyRevoked {
        /// The key.
        key: KeyId,
        /// Why.
        why: Why,
    },
    /// A request signed by a key that has run out. At most once an hour per key.
    KeyExpired {
        /// The key.
        key: KeyId,
    },
    /// A signed request was presented twice, after a valid signature. At most once an hour per
    /// key.
    ReplayDetected {
        /// The key.
        key: KeyId,
    },
}

/// What an event is written as in its row's body: strings and names, never a secret.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
enum Shown {
    KeyApproved { key: String, kind: &'static str, rights: String, scope: &'static str },
    KeyRevoked { key: String, why: &'static str },
    KeyExpired { key: String },
    ReplayDetected { key: String },
}

impl Audit {
    /// The name the log stores: noun, a dot, the past-tense verb.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::KeyApproved { .. } => "key.approved",
            Self::KeyRevoked { .. } => "key.revoked",
            Self::KeyExpired { .. } => "key.expired",
            Self::ReplayDetected { .. } => "replay.detected",
        }
    }

    /// The key this is about.
    pub const fn key(&self) -> KeyId {
        match self {
            Self::KeyApproved { key, .. }
            | Self::KeyRevoked { key, .. }
            | Self::KeyExpired { key }
            | Self::ReplayDetected { key } => *key,
        }
    }

    /// Whether the event is written at most once an hour per key, because anyone who can sign
    /// with the key can repeat what causes it.
    pub const fn hourly(&self) -> bool {
        matches!(self, Self::KeyExpired { .. } | Self::ReplayDetected { .. })
    }

    /// The row's body.
    pub(crate) fn body(&self) -> Result<String, serde_json::Error> {
        let shown = match self {
            Self::KeyApproved { key, kind, rights, scope } => Shown::KeyApproved {
                key: key.to_string(),
                kind: kind.name(),
                rights: rights.stored(),
                scope: match scope {
                    Scope::Account(_) => "account",
                    Scope::Site(_) => "site",
                },
            },
            Self::KeyRevoked { key, why } => Shown::KeyRevoked { key: key.to_string(), why: why.name() },
            Self::KeyExpired { key } => Shown::KeyExpired { key: key.to_string() },
            Self::ReplayDetected { key } => Shown::ReplayDetected { key: key.to_string() },
        };
        serde_json::to_string(&shown)
    }
}

/// An event as listed: when it happened, its name and its body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Entry {
    /// The account.
    pub account: Id<AccountKind>,
    /// When.
    pub at: Timestamp,
    /// The name.
    pub name: String,
    /// The body, as JSON.
    pub body: String,
}

impl TryFrom<&Row> for Entry {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            account: fields.id(table::AuthEvent::AccountId)?,
            at: fields.time(table::AuthEvent::At)?,
            name: fields.text(table::AuthEvent::Name)?,
            body: fields.text(table::AuthEvent::Body)?,
        })
    }
}
