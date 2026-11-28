//! Accounts.

use db::Row;
use jiff::Timestamp;

use crate::Unreadable;
use crate::fields::Fields;
use crate::id::{Account as Kind, Id};
use crate::table;

/// A person signed in to uses.ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Account {
    /// Its id.
    pub id: Id<Kind>,
    /// When it was made.
    pub created: Timestamp,
}

/// An account to add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct New {
    /// Its id, drawn by the caller.
    pub id: Id<Kind>,
    /// What time it is.
    pub at: Timestamp,
}

impl TryFrom<&Row> for Account {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        Ok(Self { id: fields.id(table::Account::Id)?, created: fields.time(table::Account::CreatedAt)? })
    }
}
