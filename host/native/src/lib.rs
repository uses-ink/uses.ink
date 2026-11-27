//! The builder's capabilities on a local disk: a folder or a git repository ([`Repository`], at
//! one commit, with dates from its history) read into a [`Snapshot`](model::Snapshot),
//! blob stores that implement [`engine::Blobs`], [`Disk`], a cache in a folder that
//! implements [`engine::Cache`], and [`Grammars`], which highlight code for a
//! theme through arborium's tree-sitter grammars (decision 0039).
//!
//! [`Folder::read`] walks a folder in byte order of its names, so the same folder always gives
//! the same snapshot. It skips `.git` and `.jj` and nothing else on its own: which files are part
//! of the site is the engine's decision (`file.ignore`). A folder holding the build's own output
//! (named by the caller, or holding the manifest `ink build` wrote) is skipped, as is the cache's
//! folder ([`Disk::LABEL`]), and so is a link to a folder, whose folder is read at its own
//! path; each skip is returned, so it can be said.
//! Everything it cannot read is a typed [`Refused`], never a file left out: a link out of the
//! folder, a name that is not UTF-8, a socket or pipe, and each budget of architecture.md section
//! 13. A file is opened once and the handle says what it is, so a file swapped for a pipe after
//! it was listed cannot block the read; a file linked twice is read once.
//!
//! A file's modification time is a bound, never a date: the bytes were written no later than it,
//! and copying or checking out a file moves it. So both facts of every file are
//! [`Known::NotAfter`](model::Known::NotAfter) from [`Provenance::Filesystem`](model::Provenance).

mod blob;
mod budget;
mod cache;
mod error;
mod folder;
mod git;
mod handle;
mod highlight;
mod read;

pub use blob::{Directory, Memory, Store, StoreError};
pub use budget::Budget;
pub use cache::Disk;
pub use error::Refused;
pub use folder::Folder;
pub use git::{Limits as GitLimits, Repository, Revision};
pub use highlight::{GrammarError, Grammars};
pub use read::{Read, Skip, Skipped};
