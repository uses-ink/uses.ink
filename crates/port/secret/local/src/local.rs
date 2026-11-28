//! The adapter.

use std::fmt;
use std::path::Path;

use logging::Field as _;
use secret::{Binding, Plain, Sealed, Secret};
use secret_seal::{Opening, Seal, Sealing};

use crate::Error;
use crate::key::{KeyFile, Origin};

type Failure = secret::Error<Error>;

/// Seals credentials under a key kept in a file.
pub struct Local {
    seal: Seal,
}

impl fmt::Debug for Local {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Local(<key redacted>)")
    }
}

impl Local {
    /// The key file's name, beside the database.
    pub const KEY: &'static str = "secret.key";

    /// The adapter keyed by the file `path`, which is made with a new key if it is missing.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        match KeyFile::open(path) {
            Ok((key, origin)) => {
                logging::info!(
                    name: logging::event::KEY_OPENED,
                    key_file = logging::Located(path).field(),
                    outcome = if origin == Origin::Made { "made" } else { "read" },
                );
                Ok(Self { seal: Seal::new(key) })
            }
            Err(error) => {
                logging::error!(
                    name: logging::event::KEY_REFUSED,
                    key_file = logging::Located(path).field(),
                    error = logging::Failure(&error).field(),
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
        KeyFile::generate().map(|key| Self { seal: Seal::new(key) })
    }
}

impl Secret for Local {
    type Error = Error;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn seal(&self, plain: &Plain, binding: &Binding) -> Result<Sealed, Failure> {
        let sealed = self.seal.seal(plain, binding).map_err(|error| match error {
            Sealing::Random { source } => Failure::Backend { source: Error::Random { source } },
            // the cipher only fails on a message far past any credential's size; that is a backend
            // failure, not a value that will not open
            Sealing::Cipher => Failure::Backend { source: Error::Seal },
        })?;
        logging::debug!(name: logging::event::SECRET_SEALED, bytes = logging::Size::from(sealed.as_ref().len()).field());
        Ok(sealed)
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn open(&self, sealed: &Sealed, binding: &Binding) -> Result<Plain, Failure> {
        match self.seal.open(sealed, binding) {
            Ok(plain) => {
                logging::debug!(name: logging::event::SECRET_OPENED, outcome = "opened");
                Ok(plain)
            }
            Err(opening) => {
                // altered, or bound to another row: said by outcome only, never by what it held
                let outcome = if opening == Opening::Malformed { "malformed" } else { "refused" };
                logging::warn!(name: logging::event::SECRET_OPENED, outcome);
                Err(opening.into())
            }
        }
    }
}
