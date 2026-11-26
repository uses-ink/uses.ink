//! Blob stores: bytes by their hash. [`Memory`] for one process, [`Directory`] for a folder of
//! `<dir>/<hash>` files that outlives it.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use engine::{BlobMissing, Blobs};
use model::Hash;

/// A blob store that takes new blobs.
pub trait Store: Blobs {
    /// Stores `bytes` under their hash and returns it. Storing a blob already held changes
    /// nothing.
    fn put(&mut self, bytes: Vec<u8>) -> Result<Hash, StoreError>;
}

/// A blob that could not be stored.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// The store's folder could not be made.
    Create { path: PathBuf, source: std::io::Error },
    /// A blob's file could not be written.
    Write { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for StoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create { path, .. } => write!(out, "the blob folder {} could not be made", path.display()),
            Self::Write { path, .. } => write!(out, "the blob {} could not be written", path.display()),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Create { source, .. } | Self::Write { source, .. } => Some(source),
        }
    }
}

/// Blobs held in memory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Memory(BTreeMap<Hash, Vec<u8>>);

impl Memory {
    /// Stores `bytes` and returns their hash. Memory takes every blob.
    pub fn insert(&mut self, bytes: Vec<u8>) -> Hash {
        let hash = Hash::of(&bytes);
        self.0.entry(hash).or_insert(bytes);
        hash
    }

    /// How many blobs it holds.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether it holds no blob.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Blobs for Memory {
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing> {
        self.0.get(hash).map(|bytes| Cow::Borrowed(bytes.as_slice())).ok_or(BlobMissing { hash: *hash })
    }
}

impl Store for Memory {
    fn put(&mut self, bytes: Vec<u8>) -> Result<Hash, StoreError> {
        Ok(self.insert(bytes))
    }
}

impl Extend<Vec<u8>> for Memory {
    /// Stores each blob under its own hash, computed here.
    fn extend<I: IntoIterator<Item = Vec<u8>>>(&mut self, blobs: I) {
        for bytes in blobs {
            self.insert(bytes);
        }
    }
}

impl FromIterator<Vec<u8>> for Memory {
    fn from_iter<I: IntoIterator<Item = Vec<u8>>>(blobs: I) -> Self {
        let mut memory = Self::default();
        memory.extend(blobs);
        memory
    }
}

/// Blobs as files in one folder, each named by its hash.
///
/// A blob is written to a temporary name and renamed into place, so a reader never sees half a
/// blob. A read checks the bytes against their name: a file that does not hash to its name is
/// reported missing, as is one that cannot be read, since [`Blobs`] has one way to fail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    root: PathBuf,
}

impl Directory {
    /// The store in `root`, made if it does not exist.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root).map_err(|source| StoreError::Create { path: root.clone(), source })?;
        Ok(Self { root })
    }

    /// The file a blob is stored in.
    pub fn path(&self, hash: &Hash) -> PathBuf {
        self.root.join(hash.to_string())
    }
}

impl Blobs for Directory {
    fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing> {
        let missing = BlobMissing { hash: *hash };
        let bytes = std::fs::read(self.path(hash)).map_err(|_unreadable| missing)?;
        if Hash::of(&bytes) == *hash { Ok(Cow::Owned(bytes)) } else { Err(missing) }
    }
}

impl Store for Directory {
    fn put(&mut self, bytes: Vec<u8>) -> Result<Hash, StoreError> {
        let hash = Hash::of(&bytes);
        let path = self.path(&hash);
        if path.is_file() {
            return Ok(hash);
        }
        let partial = self.root.join(format!(".{hash}.partial"));
        std::fs::write(&partial, &bytes).map_err(|source| StoreError::Write { path: partial.clone(), source })?;
        std::fs::rename(&partial, &path).map_err(|source| StoreError::Write { path: path.clone(), source })?;
        Ok(hash)
    }
}
