//! Who is calling: a typed credential, checked through one port.
//!
//! An [`Offered`] credential is what a request presents, already parsed from its bytes. A
//! [`Credentials`] adapter says what it speaks for ([`Verdict`]). The one adapter today is
//! [`crate::Signatures`], for the `Ink-Sig-1` profile of decision 0068; the passkey a person
//! signs in with in a browser is the next slice, and [`Offered::Passkey`] holds its place: it is
//! refused with the same generic answer as a signature that does not verify, so nothing is
//! half-built behind it.
//!
//! **Cookies and signatures never meet.** The API host reads signatures and ignores cookies;
//! the dashboard host reads cookies and accepts no signature. So the API has no ambient
//! credential: no CSRF, no CORS. A cookie is never an [`Offered`].

use std::error::Error;

use control::keys::{Effective, Kind};
use control::{AccountKind, Id};
use jiff::Timestamp;
use sig::{KeyId, Method, Signed, Target};

use crate::fault::Denied;

/// What a request presents to be known by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Offered {
    /// An `Ink-Sig-1` header: a device key, a deploy key or an operator key signed the request.
    Signed(Signed),
    /// A WebAuthn assertion. **Reserved** for the passkey slice of decision 0068: documented,
    /// typed, and refused by every verifier until that code exists.
    Passkey,
}

/// What the verifier needs of the request besides the credential: the method and the canonical
/// target. The audience is the server's own configuration and never the `Host` header.
#[derive(Clone, Copy, Debug)]
pub struct Presented<'a> {
    /// The method.
    pub method: &'a Method,
    /// The canonical target.
    pub target: &'a Target,
}

/// How a principal was proved.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Via {
    /// A line of the operator's `operator-keys` file.
    Operator {
        /// Its label.
        label: String,
    },
    /// A key the account holds.
    Key {
        /// The key.
        id: KeyId,
        /// Device or deploy.
        kind: Kind,
        /// Its label.
        label: String,
    },
}

impl Via {
    /// The key id, for a key of the database.
    pub const fn key(&self) -> Option<KeyId> {
        match self {
            Self::Key { id, .. } => Some(*id),
            Self::Operator { .. } => None,
        }
    }
}

/// Who a credential speaks for, and what it may do.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Principal {
    /// The account.
    pub account: Id<AccountKind>,
    /// How it was proved.
    pub via: Via,
    /// What it may do: its own grants, narrowed by the person's for a device key.
    pub effective: Effective,
    /// When the credential ends, when it does.
    pub expires: Option<Timestamp>,
}

/// What a credential check found.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verdict {
    /// The credential speaks for this principal.
    Granted(Principal),
    /// The signature is valid and the key is one waiting to be enrolled: it may read its own
    /// outcome and nothing else.
    Enrolling(KeyId),
    /// It speaks for nobody, and why, in the one shape the reply has.
    Refused(Denied),
}

/// Which principal a credential speaks for. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a session store on Workers has futures that are not Send")]
pub trait Credentials {
    /// Why the check could not be made.
    type Error: Error + 'static;

    /// What `offered`, made for `at`, speaks for.
    async fn verify(&self, offered: &Offered, at: &Presented<'_>) -> Result<Verdict, Self::Error>;
}
