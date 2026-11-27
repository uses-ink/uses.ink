//! Who may approve a key: the port, and the one adapter there is today.
//!
//! Approving a key gives a machine a credential, so it is the step an attacker most wants. The
//! design (decision 0068, section 8) has a person approve with a **passkey**: a user-verifying
//! WebAuthn assertion on the dashboard host. That slice is not built. Until it is, the
//! **bootstrap** adapter is a signed-in session cookie on the dashboard host ([`Strength::Session`]):
//! no better than the sign-in link that made the session, and marked so in the type, so the day
//! the passkey adapter arrives, a policy can ask for [`Strength::Passkey`] and refuse the rest.
//! The operator's key is not an approver: it signs requests, it does not enrol more keys.

use control::keys::Rights;
use control::{AccountKind, Id};
use jiff::Timestamp;

use crate::reply::Request;

/// How strongly the approver proved they are present.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Strength {
    /// A session cookie: the bootstrap path, as strong as the sign-in link.
    Session,
    /// A user-verifying passkey assertion. **Reserved**: no adapter makes it yet.
    Passkey,
}

/// A person who may approve, and what they may give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Approving {
    /// The account.
    pub account: Id<AccountKind>,
    /// The most they may give a key: their own rights.
    pub rights: Rights,
    /// How they proved it.
    pub strength: Strength,
}

/// Which person a request is from. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a session store on Workers has futures that are not Send")]
pub trait Approver {
    /// Why the check could not be made.
    type Error: std::error::Error + 'static;

    /// The person `request` is from, or `None` when it is from nobody who may approve.
    async fn approver(&self, request: &Request, now: Timestamp) -> Result<Option<Approving>, Self::Error>;
}
