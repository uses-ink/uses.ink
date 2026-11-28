//! The secret port with a local key file (decisions 0031, 0045).
//!
//! The key is 32 random bytes in a file next to the database, readable by its owner only. The
//! adapter refuses to start on a key file that other users can read, or that is not 32 bytes: a
//! loose permission is a typed error naming the file and the mode, not a warning. Credentials
//! are sealed with XChaCha20-Poly1305 under a fresh 24-byte random nonce, with the row id as
//! associated data.
//!
//! Sealed layout: one version byte (`1`), the 24-byte nonce, the ciphertext and its 16-byte tag.

mod error;
mod key;
mod local;

pub use error::Error;
pub use local::Local;
