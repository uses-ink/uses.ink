//! The adapter.

use std::fmt;
use std::path::Path;

use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{OsRng, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce, aead::Aead};
use secret::{Binding, Plain, Sealed, Secret};
use trace::Field as _;

use crate::Error;
use crate::key::{Key, Origin};

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
        let path = path.as_ref();
        match Key::open(path) {
            Ok((key, origin)) => {
                trace::info!(
                    name: trace::event::KEY_OPENED,
                    key_file = trace::Located(path).field(),
                    outcome = if origin == Origin::Made { "made" } else { "read" },
                );
                Ok(Self { key })
            }
            Err(error) => {
                trace::error!(
                    name: trace::event::KEY_REFUSED,
                    key_file = trace::Located(path).field(),
                    error = trace::Failure(&error).field(),
                );
                Err(error)
            }
        }
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

    /// A sealed value that is not one: said, and the error to return.
    fn malformed() -> Failure {
        trace::warn!(name: trace::event::SECRET_OPENED, outcome = "malformed");
        Failure::Malformed
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
        trace::debug!(name: trace::event::SECRET_SEALED, bytes = trace::Size::from(bytes.len()).field());
        Ok(Sealed::from(bytes))
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn open(&self, sealed: &Sealed, binding: &Binding) -> Result<Plain, Failure> {
        let bytes = sealed.as_ref();
        let (&version, rest) = bytes.split_first().ok_or_else(Self::malformed)?;
        if version != Self::VERSION || rest.len() < Self::NONCE + Self::TAG {
            return Err(Self::malformed());
        }
        let (nonce, ciphertext) = rest.split_at(Self::NONCE);
        let payload = Payload { msg: ciphertext, aad: binding.as_ref() };
        let opened = self.cipher().decrypt(XNonce::from_slice(nonce), payload).map_err(|_| {
            // altered, or bound to another row: said by outcome only, never by what it held
            trace::warn!(name: trace::event::SECRET_OPENED, outcome = "refused");
            Failure::Refused
        })?;
        trace::debug!(name: trace::event::SECRET_OPENED, outcome = "opened");
        Ok(Plain::from(opened))
    }
}
