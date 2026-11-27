//! How an account proves it is itself.

use db::Row;
use jiff::Timestamp;
use mail::Address;

use crate::Unreadable;
use crate::fields::Fields;
use crate::id::{Account as AccountKind, Factor as Kind, Id};
use crate::table;

/// A way to sign in, as the account has it. Only the email link exists today. A passkey and a
/// device key are later variants; nothing that matches on a credential should assume the list
/// is closed, and nothing here holds a password or anything that can be reversed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Credential {
    /// A one-time link sent to this address, which the account can receive mail at.
    Link {
        /// The address.
        address: Address,
    },
}

/// The kinds of [`Credential`], as the column stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sort {
    /// [`Credential::Link`].
    Link,
}

impl Sort {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 1] = [("link", Self::Link)];

    /// The name this kind is stored as.
    pub(crate) fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("link", |(name, _)| name)
    }
}

impl Credential {
    /// The kind of this credential.
    pub const fn sort(&self) -> Sort {
        match self {
            Self::Link { .. } => Sort::Link,
        }
    }
}

/// A credential an account holds.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Factor {
    /// Its id.
    pub id: Id<Kind>,
    /// The account that holds it.
    pub account: Id<AccountKind>,
    /// What it is.
    pub credential: Credential,
    /// When it was added.
    pub created: Timestamp,
}

impl TryFrom<&Row> for Factor {
    type Error = Unreadable;

    fn try_from(row: &Row) -> Result<Self, Unreadable> {
        let fields = Fields(row);
        let credential = match fields.name(table::Factor::Kind, &Sort::NAMES)? {
            Sort::Link => Credential::Link { address: fields.address(table::Factor::Address)? },
        };
        Ok(Self {
            id: fields.id(table::Factor::Id)?,
            account: fields.id(table::Factor::AccountId)?,
            credential,
            created: fields.time(table::Factor::CreatedAt)?,
        })
    }
}
