//! The adapter.

use std::fmt;

use logging::Field as _;
use secret::{Binding, Plain, Sealed, Secret};
use secret_seal::{Key, Opening, Seal};

use crate::Error;

type Failure = secret::Error<Error>;

/// Seals credentials under a key from a Workers secret binding, and opens what an older key sealed.
pub struct Worker {
    current: Seal,
    previous: Option<Seal>,
}

impl fmt::Debug for Worker {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Worker(<key redacted>, previous: {})", self.previous.is_some())
    }
}

impl Worker {
    /// The binding that holds the key.
    pub const KEY: &'static str = "SEALING_KEY";

    /// The binding that holds the key a rotation replaces.
    pub const PREVIOUS: &'static str = "SEALING_KEY_PREVIOUS";

    /// The adapter keyed by `key`.
    pub const fn new(key: Key) -> Self {
        Self { current: Seal::new(key), previous: None }
    }

    /// The adapter keyed by 64 hexadecimal digits. `binding` names where they came from, for the error.
    pub fn from_hex(text: &str, binding: Option<&str>) -> Result<Self, Error> {
        Key::from_hex(text).map(Self::new).map_err(|source| Error::Key { binding: binding.map(str::to_owned), source })
    }

    /// An adapter with a new key that is kept nowhere, so what it seals dies with it. For tests.
    pub fn ephemeral() -> Result<Self, Error> {
        Key::generate().map(Self::new).map_err(|source| Error::Random { source })
    }

    /// The same adapter, which also opens what the key `text` sealed (a rotation): 64 hexadecimal digits.
    pub fn previous(mut self, text: &str, binding: Option<&str>) -> Result<Self, Error> {
        let key = Key::from_hex(text).map_err(|source| Error::Key { binding: binding.map(str::to_owned), source })?;
        self.previous = Some(Seal::new(key));
        Ok(self)
    }

    /// The adapter keyed by the secret binding `name` of a Worker's environment.
    #[cfg(target_arch = "wasm32")]
    pub fn from_env(env: &worker::Env, name: &str) -> Result<Self, Error> {
        Self::from_hex(&Self::read(env, name)?, Some(name))
    }

    /// The same adapter, which also opens what the key of the secret binding `name` sealed.
    #[cfg(target_arch = "wasm32")]
    pub fn rotating(self, env: &worker::Env, name: &str) -> Result<Self, Error> {
        self.previous(&Self::read(env, name)?, Some(name))
    }

    /// The text of the secret binding `name`.
    #[cfg(target_arch = "wasm32")]
    fn read(env: &worker::Env, name: &str) -> Result<String, Error> {
        env.secret(name)
            .map(|secret| secret.to_string())
            .map_err(|source| Error::Binding { name: name.to_owned(), source })
    }

    /// Says how an open ended, as an outcome and never what was held.
    fn said(opened: &Result<(Plain, &'static str), Opening>) {
        match opened {
            Ok((_, key)) => logging::debug!(name: logging::event::SECRET_OPENED, outcome = "opened", key),
            Err(Opening::Malformed) => logging::warn!(name: logging::event::SECRET_OPENED, outcome = "malformed"),
            Err(Opening::Refused) => logging::warn!(name: logging::event::SECRET_OPENED, outcome = "refused"),
        }
    }

    /// Opens with the current key, then with the previous one if there is one and the current key refused.
    fn attempt(&self, sealed: &Sealed, binding: &Binding) -> Result<(Plain, &'static str), Opening> {
        match (self.current.open(sealed, binding), &self.previous) {
            (Ok(plain), _) => Ok((plain, "current")),
            (Err(Opening::Refused), Some(previous)) => previous.open(sealed, binding).map(|plain| (plain, "previous")),
            (Err(opening), _) => Err(opening),
        }
    }
}

impl Secret for Worker {
    type Error = Error;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for the adapters that wait; sealing does not")]
    async fn seal(&self, plain: &Plain, binding: &Binding) -> Result<Sealed, Failure> {
        let sealed =
            self.current.seal(plain, binding).map_err(|source| Failure::Backend { source: Error::Seal { source } })?;
        logging::debug!(name: logging::event::SECRET_SEALED, bytes = logging::Size::from(sealed.as_ref().len()).field());
        Ok(sealed)
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for the adapters that wait; opening does not")]
    async fn open(&self, sealed: &Sealed, binding: &Binding) -> Result<Plain, Failure> {
        let opened = self.attempt(sealed, binding);
        Self::said(&opened);
        opened.map(|(plain, _)| plain).map_err(Failure::from)
    }
}
