//! One-time tokens: what they are for, and what a call that makes or uses one returns.

use jiff::Timestamp;
use mail::Address;

use super::{Proof, Session};
use crate::id::{Account as AccountKind, Id};

/// What a one-time token is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Purpose {
    /// Signs an account in.
    Login,
    /// The first-run link: signs the owner in, and makes the owner's account when there is none.
    Admin,
}

impl Purpose {
    /// The names the column stores.
    pub(crate) const NAMES: [(&'static str, Self); 2] = [("login", Self::Login), ("admin", Self::Admin)];

    /// The name this purpose is stored as, and shown as.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("login", |(name, _)| name)
    }
}

/// A one-time token that was just made. It holds the only copy of the secret there will be.
#[derive(Debug)]
#[non_exhaustive]
pub struct Issued {
    /// The secret, to put in a link.
    pub proof: Proof,
    /// When it stops working.
    pub expires: Timestamp,
    /// The address it is for, when it is for one.
    pub address: Option<Address>,
}

/// What asking for a sign-in link came to.
#[derive(Debug)]
#[non_exhaustive]
pub enum Requested {
    /// A link was made. The caller sends it to the address.
    Issued(Issued),
    /// The address has no account and the sign-up policy makes none. Nothing was stored.
    Declined,
}

/// What is known of a link that has not been used: what it is for and when it stops working.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Pending {
    /// What it is for.
    pub purpose: Purpose,
    /// When it stops working.
    pub expires: Timestamp,
}

/// A successful sign-in.
#[derive(Debug)]
#[non_exhaustive]
pub struct Entered {
    /// The session that was made.
    pub session: Session,
    /// Its secret: shown to the person once, and kept nowhere.
    pub secret: Proof,
    /// The account that is signed in.
    pub account: Id<AccountKind>,
    /// Whether the account was made by this sign-in.
    pub created: bool,
}

/// What a person presents to sign in: the link's token and, for the first-run link, the address
/// to give the owner's account.
#[derive(Clone, Copy, Debug)]
pub struct Presented<'a> {
    /// The token from the link.
    pub proof: &'a Proof,
    /// The address for the owner's account; ignored by other links.
    pub address: Option<&'a Address>,
}
