//! The edge's ports over the folders `ink build` writes: a [`Deploys`] resolves a host to the
//! deploy in a folder, and [`Files`] reads that deploy's blobs from its files.
//!
//! A folder is a deploy when it holds `.ink-manifest.json`. Blobs are reached only through the
//! `files` list of that manifest, never from a request path, so the manifest and its record can
//! not be served and no request can name a file the build did not write. A link is refused
//! wherever a path is followed.
//!
//! The readers are the `files` feature, on by default; without it the crate has the manifest
//! checks only ([`Unchecked`], [`Checked`]) and no tokio, so an API on a Worker builds it.
//!
//! The manifest is read again when its modification time or length changes, so a rebuild into
//! the folder goes live on the next request without a restart.

#[cfg(feature = "files")]
mod blobs;
#[cfg(feature = "files")]
mod deploys;
mod error;
mod written;

#[cfg(feature = "files")]
pub use blobs::{Chunked, File, Files};
#[cfg(feature = "files")]
pub use deploys::{Deploys, Mode, Placed};
pub use error::Error;
pub use written::{Checked, MANIFEST, Unchecked};
