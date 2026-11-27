//! What the local builder fails with.

use std::fmt;

use engine::BaseUrlError;
use model::Hash;
use model::record::PluginId;

/// A failure of the local builder, as opposed to a site that does not build. `E` is the blob
/// adapter's error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// A blob of the deploy could not be stored.
    Blob(blob::Error<E>),
    /// The request's base URL is not one the engine serves under.
    Base {
        /// What the engine said.
        source: BaseUrlError,
    },
    /// `ink.toml` pins `site.format` to a format this builder does not hold.
    Format {
        /// The format's plugin id.
        id: PluginId,
    },
    /// The manifest names a blob that neither the render nor the snapshot holds: a bug in the
    /// engine, never a user's mistake.
    Missing {
        /// The blob.
        blob: Hash,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blob(_) => out.write_str("a blob of the deploy could not be stored"),
            Self::Base { .. } => out.write_str("the base URL is not one a site is served under"),
            Self::Format { id } => write!(out, "this builder holds no format {id}"),
            Self::Missing { blob } => write!(out, "the manifest names {blob}, which the build did not make"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Blob(source) => Some(source),
            Self::Base { source } => Some(source),
            Self::Format { .. } | Self::Missing { .. } => None,
        }
    }
}
