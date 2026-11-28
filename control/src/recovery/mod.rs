//! Recovery codes (decision 0085, `docs/auth.md` section 9): ten codes of 128 random bits, shown
//! once when they are made, kept as digests, each spent once to register a new passkey.
//!
//! - **A code** ([`Code`]) is 26 Crockford base32 characters and a check character, in groups of
//!   four, so a typo is found before it counts as an attempt. It prints as `Code(<redacted>)`, is
//!   zeroed when dropped and has no `Clone`; [`Code::digest`] is what is stored.
//! - **The store** ([`Codes`]) mints a set (deleting the old one in the same batch) and spends a
//!   code by one conditional update that only one caller wins. Every mint and every use is an event
//!   in the audit table, and none names a code.
//! - **The table** is made by [`Entry`], a migration to apply after the passkeys'.

mod code;
mod error;
mod migration;
mod store;

pub use code::{Code, CodeError};
pub use error::Error;
pub use migration::Entry;
pub use store::{Audit, Codes, Mint};
