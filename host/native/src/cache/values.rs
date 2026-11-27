//! The cache's folder of values: bytes by their hash, one file each.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use engine::{BlobMissing, Blobs};
use model::Hash;

use crate::blob::StoreError;

/// The cache's values as files in one folder, each named by its hash.
///
/// A blob is written to a temporary name and renamed into place, so a reader never sees half a
/// blob. A read checks the bytes against their name: a file that does not hash to its name is
/// reported missing, as is one that cannot be read, since [`Blobs`] has one way to fail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Values {
    root: PathBuf,
}

impl Values {
    /// The store in `root`, made by the first write rather than now.
    pub(super) const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The file a blob is stored in.
    pub(super) fn path(&self, hash: &Hash) -> PathBuf {
        self.root.join(hash.to_string())
    }

    /// Its folder.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    /// Stores `bytes` under their hash, from a shared reference: a
    /// write only adds a file, which another writer of the same blob adds with the same bytes.
    pub(super) fn write(&self, bytes: &[u8]) -> Result<Hash, StoreError> {
        let hash = Hash::of(bytes);
        let path = self.path(&hash);
        if !path.is_file() {
            Partial::new(&self.root, &hash.to_string()).write(bytes, &path)?;
        }
        Ok(hash)
    }
}

impl Blobs for Values {
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing> {
        let missing = BlobMissing { hash: *hash };
        let bytes = std::fs::read(self.path(hash)).map_err(|_unreadable| missing)?;
        if Hash::of(&bytes) == *hash { Ok(Cow::Owned(bytes)) } else { Err(missing) }
    }
}

/// A file on its way into place: written under a name no other writer uses (the process and a
/// counter), then renamed, so a reader in any process sees the whole file or none of it, and
/// two processes writing the same file never write into one temporary file.
#[derive(Debug)]
pub(super) struct Partial {
    path: PathBuf,
}

impl Partial {
    /// A temporary file in `folder` for the file named `name`.
    pub(super) fn new(folder: &Path, name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let count = NEXT.fetch_add(1, Ordering::Relaxed);
        Self { path: folder.join(format!(".{name}.{}.{count}.partial", std::process::id())) }
    }

    /// Writes `bytes` and renames them to `path`.
    pub(super) fn write(self, bytes: &[u8], path: &Path) -> Result<(), StoreError> {
        std::fs::write(&self.path, bytes).map_err(|source| StoreError::Write { path: self.path.clone(), source })?;
        std::fs::rename(&self.path, path).map_err(|source| StoreError::Write { path: path.to_path_buf(), source })
    }
}
