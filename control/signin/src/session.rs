//! Sessions.

use db::Row;
use jiff::Timestamp;

use super::Ends;
use control_rows::unreadable::Unreadable;
use control_rows::fields::Fields;
use control_rows::id::{Account as AccountKind, Id, Session as Kind};
use control_rows::table;

/// A signed-in session. Its secret is not here: only the person who was given it has the
/// secret, and the database has a digest of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Session {
    /// Its id, which names it in a list and is no use to sign in with.
    pub id: Id<Kind>,
    /// The account it speaks for.
    pub account: Id<AccountKind>,
    /// When it began.
    pub created: Timestamp,
    /// When it was last used.
    pub seen: Timestamp,
    /// When it ends if it is not used again.
    pub expires: Timestamp,
    /// The latest it can end.
    pub limit: Timestamp,
}

/// A person who just proved a passkey: who they are, and which passkey proved it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Passed {
    /// The account the passkey belongs to.
    pub account: Id<AccountKind>,
    /// The passkey's row id.
    pub passkey: Id<control_rows::id::Factor>,
}

impl Session {
    /// The columns a read of a session selects, digest first.
    pub(crate) const COLUMNS: [table::Session; 7] = [
        table::Session::Digest,
        table::Session::Id,
        table::Session::AccountId,
        table::Session::CreatedAt,
        table::Session::SeenAt,
        table::Session::ExpiresAt,
        table::Session::LimitAt,
    ];

    /// The two end times.
    pub const fn ends(&self) -> Ends {
        Ends { expires: self.expires, limit: self.limit }
    }
}

impl TryFrom<&Row> for Session {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self {
            id: fields.id(table::Session::Id)?,
            account: fields.id(table::Session::AccountId)?,
            created: fields.time(table::Session::CreatedAt)?,
            seen: fields.time(table::Session::SeenAt)?,
            expires: fields.time(table::Session::ExpiresAt)?,
            limit: fields.time(table::Session::LimitAt)?,
        })
    }
}
