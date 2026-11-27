//! Signing in (decision 0066): one-time email links, sessions, the first-run admin link and the
//! policy that says who may make an account, over the row port alone.
//!
//! - **No password exists.** An account's [`Credential`]s are typed, and the only one today is
//!   [`Credential::Link`]; a passkey and a device key are later variants. Magic links stay as
//!   bootstrap and recovery.
//! - **Only digests are stored.** A link's token and a session's secret are 256 random bits
//!   ([`Proof`]), drawn by the caller and returned once. The rows hold their BLAKE3 digests
//!   ([`Digest`]), keyed per use so a link is no session. A [`Proof`] prints as
//!   `Proof(<redacted>)`, is zeroed when dropped and cannot be cloned.
//! - **Time and randomness are arguments.** [`SignIn`] reads no clock and draws nothing:
//!   [`Drawn`] carries what a call needs.
//! - **Settings** are one typed value ([`Settings`]) with a default for each field.
//!
//! The tables are made by [`Entry`], a migration to apply after the domain's.

mod audit;
mod challenge;
mod error;
mod factor;
mod migration;
mod proof;
mod session;
mod settings;
mod store;
mod ttl;

pub use audit::Audit;
pub use challenge::{Entered, Issued, Pending, Presented, Purpose, Requested};
pub use error::{Error, Refused};
pub use factor::{Credential, Factor, Sort};
pub use migration::Entry;
pub use crate::digest::{Digest, NotDigest};
pub use proof::{Context, Drawn, Proof, ProofError};
pub use session::{Passed, Session};
pub use settings::{Ends, Expiry, Settings, Signup, UnknownSignup};
pub use store::SignIn;
pub use ttl::{Ttl, TtlError};
