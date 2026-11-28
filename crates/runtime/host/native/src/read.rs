//! What reading a source gives, whichever source it was: the snapshot, and what the read left
//! out on the way.

use model::{Announcement, ContentPath, Notice, Snapshot, Subject};

/// A source read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Read {
    /// The files, their facts and the source's version.
    pub snapshot: Snapshot,
    /// Folders not read, in the order the walk met them.
    pub skipped: Vec<Skipped>,
    /// Files that are Git LFS pointers: the snapshot holds the pointer's text, not the file it
    /// stands for.
    pub pointers: Vec<ContentPath>,
}

impl Read {
    /// A read of `snapshot` that skipped nothing and met no pointer: for a source that holds
    /// its own files, such as an upload.
    pub const fn new(snapshot: Snapshot) -> Self {
        Self { snapshot, skipped: Vec::new(), pointers: Vec::new() }
    }

    /// What the read has to say: each folder it skipped, then each pointer it kept.
    pub fn notices(&self) -> impl Iterator<Item = Notice> + '_ {
        let pointers = self.pointers.iter().map(|path| {
            Notice::new(Announcement::SourceLfsPointer { path: path.clone() }).on(Subject::File(path.clone()))
        });
        self.skipped.iter().map(Notice::from).chain(pointers)
    }
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
    /// It is the cache ink keeps between builds: it holds the mark
    /// [`Disk::LABEL`](crate::Disk::LABEL).
    Cache,
    /// It is a link to a folder of the site, read at its own path. Following it would read the
    /// same files again under a second path, and a ladder of such links doubles the paths at
    /// every step.
    Link,
    /// It is a git submodule: its files are in another repository.
    Submodule,
}

impl From<&Skipped> for Notice {
    fn from(skipped: &Skipped) -> Self {
        let path = skipped.path.clone();
        let announcement = match skipped.reason {
            Skip::Output => Announcement::SourceOutputSkipped { path: path.clone() },
            Skip::Cache => Announcement::SourceCacheSkipped { path: path.clone() },
            Skip::Link => Announcement::SourceLinkSkipped { path: path.clone() },
            Skip::Submodule => Announcement::SourceSubmoduleSkipped { path: path.clone() },
        };
        Self::new(announcement).on(Subject::File(path))
    }
}

/// A file's first bytes, asked whether it is a Git LFS pointer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pointer<'a>(pub(crate) &'a [u8]);

impl Pointer<'_> {
    /// How every pointer starts (the Git LFS specification, v1).
    const START: &'static [u8] = b"version https://git-lfs.github.com/spec/v1\n";

    /// Pointers are at most this long.
    const MOST: usize = 1024;

    /// Whether the bytes are a pointer: short, and starting as the specification says.
    pub(crate) fn is_lfs(self) -> bool {
        self.0.len() <= Self::MOST && self.0.starts_with(Self::START)
    }
}
