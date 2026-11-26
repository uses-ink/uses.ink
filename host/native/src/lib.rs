//! The builder's capabilities on a local disk: a folder read into a [`Snapshot`](model::Snapshot),
//! and blob stores that implement [`engine::Blobs`].
//!
//! [`Folder::read`] walks a folder in byte order of its names, so the same folder always gives
//! the same snapshot. It skips `.git` and `.jj` and nothing else on its own: which files are part
//! of the site is the engine's decision (`file.ignore`). A folder the caller names as the build's
//! own output is skipped too, and the skip is returned, so it can be said. Everything it cannot
//! read is a typed [`Refused`], never a file left out: a link out of the folder, a link cycle, a
//! name that is not UTF-8, a socket, and each budget of architecture.md section 13.
//!
//! A file's modification time is a bound, never a date: the bytes were written no later than it,
//! and copying or checking out a file moves it. So both facts of every file are
//! [`Known::NotAfter`](model::Known::NotAfter) from [`Provenance::Filesystem`](model::Provenance).

mod blob;
mod budget;
mod error;
mod folder;

pub use blob::{Directory, Memory, Store, StoreError};
pub use budget::Budget;
pub use error::Refused;
pub use folder::{Folder, Read, Skipped};
