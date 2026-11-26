//! What reading a source gives, whichever source it was: the snapshot, and what the read left
//! out on the way.

use model::{ContentPath, Snapshot};

/// A source read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Read {
    /// The files, their facts and the source's version.
    pub snapshot: Snapshot,
    /// Folders not read, in the order the walk met them.
    pub skipped: Vec<Skipped>,
}

/// A folder left unread, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Skipped {
    /// The folder, or the link to one.
    pub path: ContentPath,
    /// Why it was not read.
    pub reason: Skip,
}

/// Why a folder was not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Skip {
    /// It holds the build's own output: the caller named it, or it holds the manifest
    /// [`Folder::MANIFEST`](crate::Folder::MANIFEST) that `ink build` wrote.
    Output,
    /// It is a link to a folder of the site, read at its own path. Following it would read the
    /// same files again under a second path, and a ladder of such links doubles the paths at
    /// every step.
    Link,
}
