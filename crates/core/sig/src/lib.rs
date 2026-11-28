//! The signed-request profile `Ink-Sig-1` of decision 0068 (`docs/auth.md` section 5).
//!
//! A machine (the CLI, a CI job) signs every API request with an Ed25519 key. This crate is both
//! ends of that: the **strict header** ([`Signed`]: one wire form per message), the **signed
//! text** ([`Covered`]: a context line, the audience, the method, the canonical target, the
//! BLAKE3 digest of the body, the time, the nonce and the key id), the **canonical target**
//! ([`Target`], which is checked and never repaired), the keys ([`Seed`], [`PublicKey`],
//! [`KeyId`], [`Fingerprint`]) and the signer and the strict verifier ([`Signer`],
//! [`Verifying`]).
//!
//! It reads no clock and draws nothing: the caller gives the time ([`Created`]) and the nonce
//! ([`Nonce`]), so a test signs the same bytes every time. It is the only crate of ours that
//! names `ed25519-dalek`, so a swap of the signature crate is one crate and one test run.
//!
//! There is no algorithm agility: a key's type ([`KeyType`]) is fixed when it is enrolled and the
//! request cannot name or change it.

mod alphabet;
mod covered;
mod key;
mod signed;
mod signer;
mod target;

pub use alphabet::{Base64, Crockford, Undecodable};
pub use covered::{Audience, AudienceError, Covered, Method, MethodError};
pub use key::{Fingerprint, FingerprintError, KeyError, KeyId, KeyText, KeyType, PublicKey, Seed, UnknownKeyType};
pub use signed::{Claim, Created, CreatedError, Digest, Field, Malformed, Nonce, Signature, Signed, Skew, Tampered};
pub use signer::{Forged, Parts, Signer, Verifying};
pub use target::{NotCanonical, Target};
