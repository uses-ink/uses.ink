//! Lifecycle events of passkeys, written to the same `auth_event` table as the keys' (`docs/auth.md`
//! section 13): a passkey added, removed, flagged, a sign-in. Each names the passkey row and the
//! account and never a secret. "Last used" is a date on the passkey's row, not an event.

use db::Statement;
use jiff::Timestamp;
use sea_query::{Expr, Query};
use serde_json::json;

use control_rows::id::{Account as AccountKind, Factor as FactorKind, Id};
use control_rows::table;

/// One lifecycle event of a passkey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Audit {
    /// A passkey was registered.
    Added {
        /// The passkey's row id.
        passkey: Id<FactorKind>,
    },
    /// A passkey was removed, and the sessions it made ended with it.
    Removed {
        /// The passkey's row id.
        passkey: Id<FactorKind>,
    },
    /// The signature counter did not rise. On a hardware key the sign-in was refused.
    Flagged {
        /// The passkey's row id.
        passkey: Id<FactorKind>,
        /// Whether the sign-in was refused.
        refused: bool,
    },
    /// A passkey signed in, and a session began.
    SignedIn {
        /// The passkey's row id.
        passkey: Id<FactorKind>,
    },
}

impl Audit {
    /// The name the log stores: noun, a dot, the past-tense verb.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Added { .. } => "passkey.added",
            Self::Removed { .. } => "passkey.removed",
            Self::Flagged { .. } => "passkey.flagged",
            Self::SignedIn { .. } => "passkey.signed-in",
        }
    }

    /// The passkey this is about.
    pub const fn passkey(&self) -> Id<FactorKind> {
        match self {
            Self::Added { passkey }
            | Self::Removed { passkey }
            | Self::Flagged { passkey, .. }
            | Self::SignedIn { passkey } => *passkey,
        }
    }

    /// The row's body: the event's name and the passkey, as JSON.
    fn body(&self) -> String {
        let passkey = self.passkey().to_string();
        match self {
            Self::Flagged { refused, .. } => json!({"event": "flagged", "passkey": passkey, "refused": refused}),
            Self::Added { .. } => json!({"event": "added", "passkey": passkey}),
            Self::Removed { .. } => json!({"event": "removed", "passkey": passkey}),
            Self::SignedIn { .. } => json!({"event": "signed-in", "passkey": passkey}),
        }
        .to_string()
    }

    /// The statement that writes this event for `account`.
    pub fn statement(
        &self,
        account: Id<AccountKind>,
        now: Timestamp,
    ) -> Result<Statement, sea_query::error::Error> {
        let insert = Query::insert()
            .into_table(table::AuthEvent::Table)
            .columns([
                table::AuthEvent::AccountId,
                table::AuthEvent::KeyId,
                table::AuthEvent::At,
                table::AuthEvent::Name,
                table::AuthEvent::Body,
            ])
            .values([
                Expr::val(account.to_string()),
                Expr::val(self.passkey().to_string()),
                Expr::val(now.as_millisecond()),
                Expr::val(self.name()),
                Expr::val(self.body()),
            ])?
            .to_owned();
        Ok(insert.into())
    }
}
