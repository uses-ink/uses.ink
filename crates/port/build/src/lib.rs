//! The build port of the control plane: run a build of a [`Source`] into a deploy, and say what
//! happens on the way as one typed stream of [`Event`]s (decision 0023, architecture section 15).
//!
//! A [`Build`] stores every blob the deploy's manifest names under the request's tenant before
//! it returns, and returns the [`model::Manifest`]. It does not publish, and it does not store
//! the manifest: the control plane stores that with the deploy and decides what is live. A site that does not build (an error in its files, a
//! source that cannot be read) is not a failure of the builder: the stream says why, and the
//! [`Outcome`] is [`Outcome::Refused`]. A failure of the builder itself is an [`Error`].
//!
//! The adapters are `build/process`, which runs the engine in this process, and, hosted,
//! `build/container`; the suite every adapter runs is `build/conformance`.

mod error;
mod event;
mod id;
mod port;
mod source;
mod tree;

pub use error::Error;
pub use event::{Event, Events, Status, Trigger};
pub use id::{BuildId, BuildIdError};
pub use port::{Build, Built, Outcome, Request};
pub use source::{Kind, Revision, RevisionError, Source};
pub use tree::{Leaf, Tree, TreeError};
