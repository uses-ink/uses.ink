//! Sealing and opening.

use std::fmt;

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore as _};
use secret::{Binding, Plain, Sealed};

use crate::error::{Opening, Sealing};
use crate::key::Key;

/// Seals credentials under one key.
pub struct Seal {
    key: Key,
}

impl fmt::Debug for Seal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Seal(<key redacted>)")
    }
}

impl Seal {
    /// The format version byte a sealed value starts with.
    pub const VERSION: u8 = 1;

    /// The nonce's length in bytes.
    pub const NONCE: usize = 24;

    /// The tag's length in bytes.
    pub const TAG: usize = 16;

    /// A seal under `key`.
    pub const fn new(key: Key) -> Self {
        Self { key }
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(self.key.expose().into())
    }

    /// Seals `plain` to `binding` under a fresh nonce.
    pub fn seal(&self, plain: &Plain, binding: &Binding) -> Result<Sealed, Sealing> {
        let mut nonce = [0_u8; Self::NONCE];
        OsRng.try_fill_bytes(&mut nonce).map_err(|source| Sealing::Random { source })?;
        let payload = Payload { msg: plain.expose(), aad: binding.as_ref() };
        let sealed = self.cipher().encrypt(XNonce::from_slice(&nonce), payload).map_err(|_| Sealing::Cipher)?;
        let mut bytes = Vec::with_capacity(1 + Self::NONCE + sealed.len());
        bytes.push(Self::VERSION);
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&sealed);
        Ok(Sealed::from(bytes))
    }

    /// Opens `sealed` for `binding`.
    pub fn open(&self, sealed: &Sealed, binding: &Binding) -> Result<Plain, Opening> {
        let (&version, rest) = sealed.as_ref().split_first().ok_or(Opening::Malformed)?;
        if version != Self::VERSION || rest.len() < Self::NONCE + Self::TAG {
            return Err(Opening::Malformed);
        }
        let (nonce, ciphertext) = rest.split_at(Self::NONCE);
        let payload = Payload { msg: ciphertext, aad: binding.as_ref() };
        let opened = self.cipher().decrypt(XNonce::from_slice(nonce), payload).map_err(|_| Opening::Refused)?;
        Ok(Plain::from(opened))
    }
}
