//! Why the settings could not be made into a connection configuration.

use std::fmt;
use std::path::PathBuf;

/// The https settings could not be made into a connection configuration.
#[derive(Debug)]
#[non_exhaustive]
pub enum Misconfigured {
    /// The TLS client could not be set up: a protocol version or a root the provider refuses.
    Tls(rustls::Error),
    /// The HTTP client could not be built from the settings.
    Client(reqwest::Error),
    /// A file of extra roots could not be read as PEM certificates.
    Roots {
        /// The file.
        path: PathBuf,
        /// Why.
        source: rustls::pki_types::pem::Error,
    },
}

impl fmt::Display for Misconfigured {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Roots { path, .. } => write!(out, "{} holds no readable PEM certificates", path.display()),
            Self::Tls(_) => out.write_str("the TLS client could not be configured"),
            Self::Client(_) => out.write_str("the HTTP client could not be built"),
        }
    }
}

impl std::error::Error for Misconfigured {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Tls(source) => Some(source),
            Self::Client(source) => Some(source),
            Self::Roots { source, .. } => Some(source),
        }
    }
}
