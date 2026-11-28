//! The build port in this process (decision 0045): the same pipeline `ink build` runs, over a
//! folder, with the native host's capabilities and the bundled plugins (markdown, Obsidian and
//! the quiet theme), writing the blobs of the deploy to any [`blob::Blob`].
//!
//! A build is blocking CPU work inside an async fn, which suits a self-host that runs builds on
//! a worker thread of its own. A site that does not build (an error in `ink.toml`, a file that
//! cannot be read, a diagnostic that stops the render) is an [`build::Outcome::Refused`] with
//! its diagnostics in the event stream, never an [`Error`]. A git source is read through the
//! [`Remote`] the builder is given; with none ([`Offline`]) it is [`build::Error::Unsupported`].
//!
//! The pipeline repeats what `cli/src/site.rs` does, because the CLI keeps it private; both
//! should call one shared function when the CLI next changes (decision 0045).

mod bundle;
mod error;
mod local;
mod pins;
mod pipeline;
mod reader;
mod remote;

pub use error::Error;
pub use local::Local;
pub use remote::{Offline, Remote};
