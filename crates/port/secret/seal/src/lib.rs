//! The sealing both secret adapters share (decision 0133).
//!
//! A [`Key`] is 32 bytes. A [`Seal`] over it seals a credential to the row that holds it with
//! XChaCha20-Poly1305 under a fresh 24-byte nonce and the row's id as associated data, and opens
//! what it sealed. Where the key comes from is the adapter's business: a file beside the database
//! (`secret-local`) or a Worker's secret binding (`secret-worker`). Nothing here logs or reads a
//! file; the adapters say what happened in their own words.
//!
//! Sealed layout: one version byte (`1`), the 24-byte nonce, the ciphertext and its 16-byte tag.

mod error;
mod key;
mod seal;

pub use error::{Entropy, Opening, Sealing, Unhexed};
pub use key::Key;
pub use seal::Seal;
