//! Passkeys (decision 0068, `docs/auth.md` section 4): the port a WebAuthn verifier implements,
//! the typed values of its ceremonies, and the store of credentials and ceremonies in progress.
//!
//! - **The port** is [`Passkeys`]. The control plane never names a verifier library; the adapter
//!   (`edge/api`'s `Webauthn`) does. Its rules are in the contract: discoverable credentials,
//!   user verification required, ES256, attestation `none`.
//! - **Ceremonies are values** ([`Creation`], [`Asking`], [`Created`], [`Asserted`], [`Stored`],
//!   [`Verified`]), with the browser's JSON field names, so no page builds options by hand.
//! - **The relying party** ([`Relying`]) is the id credentials are bound to and the origins that
//!   may ask. It is settings, never a constant: `app.uses.ink` hosted, the operator's own for a
//!   self-host, a test's own in tests.
//! - **The store** ([`Registry`]) keeps passkeys and ceremonies. A ceremony is the digest of a
//!   cookie secret, spent by one conditional delete and ended after five minutes, so a reply
//!   cannot be replayed or completed from another browser. Events go to the keys' audit table.
//! - **The counter policy** is [`Counter::judge`]: used only when used, flagged on a synced
//!   passkey, refused on a hardware one.
//!
//! The tables are made by [`Entry`], a migration to apply after the keys'.

mod bytes;
mod ceremony;
mod error;
mod migration;
mod port;
mod reason;
mod relying;
mod store;

pub use crate::signin::Audit;
pub use bytes::{Bytes, CredentialId};
pub use ceremony::{
    Algorithm, Asking, Asserted, Attestation, Authenticating, Claim, Class, Counter, Created, Creation, Enrolled,
    Holder, Named, Party, Person, Registering, Requirement, Selection, State, Stored, Transport, Verified,
};
pub use error::{Error, Refused};
pub use migration::Entry;
pub use port::Passkeys;
pub use reason::{Reason, Refusal};
pub use relying::{Relying, RelyingError};
pub use store::{Begin, Flow, Fresh, Passkey, Registry, Spent};
