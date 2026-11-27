//! Where a git repository's branch is now, read from the remote's own list of references
//! (decision 0067).
//!
//! [`Remotes`] is the watch's [`watch::Heads`] over `gix`: it connects to the repository the way
//! `git ls-remote` does, reads the reference list the server advertises and fetches no object.
//! The transport is the one the builder's fetch uses for the same scheme, and the operator's
//! [`model::git::Network`] setting says which schemes may be reached at all. A credential is
//! given to the transport and to nothing else.
//!
//! A connection needs a repository to run in, so one empty bare repository is kept in a folder
//! of the caller's; nothing is ever fetched into it.

mod error;
mod listing;
mod remotes;
mod tips;

pub use error::Unlisted;
pub use remotes::Remotes;
pub use tips::Missing;
