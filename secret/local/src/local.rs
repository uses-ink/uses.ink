//! The adapter.

use std::fmt;
use std::path::Path;

use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{OsRng, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use secret::{Binding, Plain, Sealed, Secret};

use crate::Error;
use crate::key::Key;

type Failure = secret::Error<Error>;

/// Seals credentials under a key kept in a file.
pub struct Local {
    key: Key,
}

impl fmt::Debug for Local {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Local(<key redacted>)")
    }
}

impl Local {
    /// The key file's name, beside the database.
    pub const KEY: &'static str = "secret.key";

    /// The format version byte a sealed value starts with.
    const VERSION: u8 = 1;
    const NONCE: usize = 24;
    const TAG: usize = 16;

    /// The adapter keyed by the file `path`, which is made with a new key if it is missing.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        Key::open(path.as_ref()).map(|key| Self { key })
    }

    /// The adapter keyed by [`Local::KEY`] in the folder of the database file `database`.
    pub fn beside(database: impl AsRef<Path>) -> Result<Self, Error> {
        let folder = database.as_ref().parent().unwrap_or_else(|| Path::new("."));
        Self::open(folder.join(Self::KEY))
    }

    /// An adapter with a new key that is kept nowhere, so what it seals dies with it. For tests.
    pub fn ephemeral() -> Result<Self, Error> {
        Key::generate().map(|key| Self { key })
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(self.key.bytes().into())
    }
}

impl Secret for Local {
    type Error = Error;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn seal(&self, plain: &Plain, binding: &Binding) -> Result<Sealed, Failure> {
        let mut nonce = [0_u8; Self::NONCE];
        OsRng.try_fill_bytes(&mut nonce).map_err(|source| Failure::Backend { source: Error::Random { source } })?;
        let payload = Payload { msg: plain.expose(), aad: binding.as_ref() };
        // the cipher only fails on a message far past any credential's size; that is a backend
        // failure, not a value that will not open
        let sealed = self
            .cipher()
            .encrypt(XNonce::from_slice(&nonce), payload)
            .map_err(|_| Failure::Backend { source: Error::Seal })?;
        let mut bytes = Vec::with_capacity(1 + Self::NONCE + sealed.len());
        bytes.push(Self::VERSION);
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&sealed);
        Ok(Sealed::from(bytes))
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn open(&self, sealed: &Sealed, binding: &Binding) -> Result<Plain, Failure> {
        let bytes = sealed.as_ref();
        let (&version, rest) = bytes.split_first().ok_or(Failure::Malformed)?;
        if version != Self::VERSION || rest.len() < Self::NONCE + Self::TAG {
            return Err(Failure::Malformed);
        }
        let (nonce, ciphertext) = rest.split_at(Self::NONCE);
        let payload = Payload { msg: ciphertext, aad: binding.as_ref() };
        let opened = self.cipher().decrypt(XNonce::from_slice(nonce), payload).map_err(|_| Failure::Refused)?;
        Ok(Plain::from(opened))
    }
}
