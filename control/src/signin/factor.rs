//! How an account proves it is itself.

use db::Row;
use jiff::Timestamp;
use mail::Address;

use crate::unreadable::Unreadable;
use crate::fields::Fields;
use crate::id::{Account as AccountKind, Factor as Kind, Id, Site as SiteKind};
use crate::table;

/// A way to sign in, as the account has it (decision 0068, `docs/auth.md` section 2). Nothing
/// that matches on a credential should assume the list is closed, and nothing here holds a
/// password or anything that can be reversed: a key is a public half, and its private half lives
/// on the machine that made it.
///
/// A [`Credential::Link`] is a row of the `factor` table. The two keys are rows of the `key`
/// table ([`crate::keys::Key::credential`]), which holds what only a key has: its rights, expiry
/// and last use.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Credential {
    /// A one-time link sent to this address, which the account can receive mail at.
    Link {
        /// The address.
        address: Address,
    },
    /// The CLI's Ed25519 key on a person's machine. It acts for that person, with what they may
    /// do narrowed to what the key was approved for.
    DeviceKey {
        /// The key.
        key: sig::KeyId,
    },
    /// A CI or deploy job's Ed25519 key: one site, and the rights it was approved with.
    DeployKey {
        /// The key.
        key: sig::KeyId,
        /// The site it is for.
        site: Id<SiteKind>,
    },
    /// A WebAuthn passkey, for people in a browser. **Reserved**: the dashboard and the passkey
    /// ceremonies are the next slice of decision 0068, so nothing makes this credential and the
    /// verifier refuses it with a typed refusal rather than guessing.
    Passkey,
}

/// The kinds of [`Credential`], as the column stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sort {
    /// [`Credential::Link`].
    Link,
    /// [`Credential::DeviceKey`].
    DeviceKey,
    /// [`Credential::DeployKey`].
    DeployKey,
    /// [`Credential::Passkey`].
    Passkey,
}

impl Sort {
    /// The names the `factor` column stores: only links are rows of that table.
    pub(crate) const NAMES: [(&'static str, Self); 1] = [("link", Self::Link)];

    /// The name this kind is written as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::DeviceKey => "device-key",
            Self::DeployKey => "deploy-key",
            Self::Passkey => "passkey",
        }
    }
}

impl Credential {
    /// The kind of this credential.
    pub const fn sort(&self) -> Sort {
        match self {
            Self::Link { .. } => Sort::Link,
            Self::DeviceKey { .. } => Sort::DeviceKey,
            Self::DeployKey { .. } => Sort::DeployKey,
            Self::Passkey => Sort::Passkey,
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
            // the column only lets `link` in, so another kind is a row from a newer version
            other => return Err(Unreadable::Name { column: "kind", found: other.name().to_owned() }),
        };
        Ok(Self {
            id: fields.id(table::Factor::Id)?,
            account: fields.id(table::Factor::AccountId)?,
            credential,
            created: fields.time(table::Factor::CreatedAt)?,
        })
    }
}
