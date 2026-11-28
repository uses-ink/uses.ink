//! The `git` capability on a local disk (decisions 0036 and 0046): a repository fetched into a
//! cache folder the host manages, then read at one commit through [`Repository`].
//!
//! [`Fetch`] is the operator's side of it. What the builder lets a source reach is its
//! [`model::git::Network`] setting, which defaults to off; what a clone may cost is [`FetchLimits`]; which
//! credentials the builder holds is [`Grants`], and a request only ever names one. gix is opened
//! isolated, with no user or system configuration, no credential helper and no prompt, so
//! nothing outside these settings changes what a fetch does. A clone is bare, in
//! `<cache>/git/<key>` where the key is [`model::git::Request::key`], and a second fetch of the same request
//! brings it up to date and to the depth asked for.
//!
//! The byte budget is watched while the pack arrives, so a remote that sends without end is cut
//! at the limit; the object budget is counted once the objects are indexed. A clone that passes a
//! budget is removed. `https://` goes through the one TLS stack of the `https` crate (decision 0072), within
//! [`Fetch::https`]: timeouts, redirects and trusted roots, with certificates always verified.

mod client;
mod cloning;
mod failed;
mod grants;
mod limits;
mod watch;

pub use client::Fetch;
pub use cloning::Cloning;
pub use failed::Failed;
pub use grants::Grants;
pub use limits::FetchLimits;
