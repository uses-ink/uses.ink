//! The edge's host-to-deploy port over the control plane (decision 0067).
//!
//! The control plane's ports are not `Send` (D1 and R2 are not), and the edge's [`edge::Sites`]
//! is, so the lookup crosses a channel the way the API's calls do: [`Back::pair`] makes a [`Fronts`],
//! which is `Send`, `Sync` and clone and answers [`edge::Sites::resolve`], and a [`Back`], which
//! owns the [`control::Control`] and answers one lookup at a time on whatever task runs it.
//!
//! A lookup maps the host to a handle under the control plane's origin, the handle to a site
//! that takes changes, the site to its live deploy and the deploy to its manifest, which is read
//! back from the blob store with every route's media type, status and headers. A manifest is
//! immutable (a deploy's id is its hash), so decoded manifests are kept for as long as there is
//! room and never go stale; only the pointer from a handle to its live deploy has a time to
//! live ([`Settings::ttl`]), which a rollback or a new deploy waits out. The blobs are not read
//! through the port: a deploy's files are the blob store's own files, so [`edge_folder::Files`]
//! serves them with its link refusal, swap check and range reads, unchanged.
//!
//! [`Fallback`] puts a folder's deploys behind the control plane's, so one server answers both.

mod fallback;
mod locate;
mod lookup;

pub use fallback::{Either, Fallback};
pub use locate::Locate;
pub use lookup::cache::Settings;
pub use lookup::caller::Fronts;
pub use lookup::error::{Chain, Failure};
pub use lookup::owner::Back;
