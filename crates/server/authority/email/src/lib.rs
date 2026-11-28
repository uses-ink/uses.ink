//! The email authority, `ink:authority-email` (decision 0094): the sign-in magic link of decision
//! 0066 as a proof that someone reads mail at an address.
//!
//! - [`Email::send`] stores a one-time proof for an address and mails its link.
//! - [`Email::confirm`] spends the proof and returns the address. It makes no session, so the
//!   proof is no sign-in; and a sign-in link is no proof, because the two are stored under different
//!   digest keys.
//! - A proof works once and for fifteen minutes ([`Email::LIFE`]), the same as a sign-in link.
//!
//! The control plane decides what the address then means; this crate only says it was read.

mod email;
mod error;

pub use email::Email;
pub use error::Error;
