//! The blob port on a directory.
//!
//! ```text
//! <root>/t/<tenant>/b/<hash>     one file per blob, named by its 64 hex digits
//! <root>/tmp/<n>.part            writes in progress, on the same file system, never listed
//! ```
//!
//! A path is built only from a validated [`blob::Tenant`] and a [`model::Hash`], so no text an
//! outsider chose is ever a path segment. A write goes to `tmp/`, is hashed as it is written,
//! synced and renamed into place, so a reader sees all of a blob or none of it. A read hashes as
//! it streams and fails at the end if the file is not what its name says. A file in a tenant's
//! folder that is not a blob is an [`Error::Stray`], never skipped. One process owns the
//! directory; the file calls block, which suits a single-machine self-host.

mod directory;
mod error;
mod read;

pub use directory::Directory;
pub use error::Error;
pub use read::Reading;
