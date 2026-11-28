//! What the Worker adapter fails with.

use std::fmt;

use secret_seal::{Entropy, Sealing, Unhexed};

/// A failure of the Worker adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The key is not 64 hexadecimal digits. It is not shown: only what is wrong with its shape.
    Key {
        /// The binding's name, when the key came from one.
        binding: Option<String>,
        /// What is wrong with it.
        source: Unhexed,
    },
    /// The binding could not be read from the Worker's environment.
    #[cfg(target_arch = "wasm32")]
    Binding {
        /// The binding's name.
        name: String,
        /// What the environment said.
        source: worker::Error,
    },
    /// Sealing failed.
    Seal {
        /// Why.
        source: Sealing,
    },
    /// The operating system gave no randomness for a key.
    Random {
        /// What it said.
        source: Entropy,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key { binding: Some(name), .. } => write!(out, "the secret binding `{name}` is not a key"),
            Self::Key { binding: None, .. } => out.write_str("the text is not a key"),
            #[cfg(target_arch = "wasm32")]
            Self::Binding { name, .. } => write!(out, "the secret binding `{name}` could not be read"),
            Self::Seal { .. } => out.write_str("the credential could not be sealed"),
            Self::Random { .. } => out.write_str("the operating system gave no randomness"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Key { source, .. } => Some(source),
            #[cfg(target_arch = "wasm32")]
            Self::Binding { source, .. } => Some(source),
            Self::Seal { source } => Some(source),
            Self::Random { source } => Some(source),
        }
    }
}
