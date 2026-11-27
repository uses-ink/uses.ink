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
    /// The remote's fetch failed in its own right, not as a repository that cannot be read.
    Remote(Box<dyn std::error::Error>),
    /// A blob of an uploaded tree is not held: the push that made the build said they were.
    Absent {
        /// The blob.
        blob: Hash,
    },
    /// An uploaded tree, or a file of it, could not be read from the blob store.
    Stored(blob::Error<E>),
    /// The uploaded tree is not a tree.
    Tree(build::TreeError),
    /// The uploaded files do not make a snapshot.
    Snapshot(model::SnapshotError),
    /// The manifest names a blob that neither the render nor the snapshot holds: a bug in the
    /// engine, never a user's mistake.
    Missing {
        /// The blob.
        blob: Hash,
        /// What the store said.
        source: engine::BlobMissing,
    },
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blob(_) => out.write_str("a blob of the deploy could not be stored"),
            Self::Absent { blob } => write!(out, "the uploaded blob {blob} is not held"),
            Self::Stored(_) => out.write_str("an uploaded blob could not be read"),
            Self::Tree(_) => out.write_str("the uploaded tree is not a tree"),
            Self::Snapshot(_) => out.write_str("the uploaded files do not make a snapshot"),
            Self::Base { .. } => out.write_str("the base URL is not one a site is served under"),
            Self::Remote(_) => out.write_str("the remote could not fetch the repository"),
            Self::Format { id } => write!(out, "this builder holds no format {id}"),
            Self::Missing { blob, .. } => write!(out, "the manifest names {blob}, which the build did not make"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Blob(source) | Self::Stored(source) => Some(source),
            Self::Tree(source) => Some(source),
            Self::Snapshot(source) => Some(source),
            Self::Base { source } => Some(source),
            Self::Missing { source, .. } => Some(source),
            Self::Remote(source) => Some(source.as_ref()),
            Self::Format { .. } | Self::Absent { .. } => None,
        }
    }
}
