//! Blob stores: bytes by their hash. [`Memory`] for one process.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

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
    Create {
        /// The folder.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A blob's file could not be written.
    Write {
        /// The blob's file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A file the cache meant to drop could not be removed, and stays.
    Remove {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create { path, .. } => write!(out, "the blob folder {} could not be made", path.display()),
            Self::Write { path, .. } => write!(out, "the blob {} could not be written", path.display()),
            Self::Remove { path, .. } => write!(out, "the cache file {} could not be removed", path.display()),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Create { source, .. } | Self::Write { source, .. } | Self::Remove { source, .. } => Some(source),
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

impl IntoIterator for Memory {
    type Item = Vec<u8>;
    type IntoIter = std::collections::btree_map::IntoValues<Hash, Vec<u8>>;

    /// Every blob's bytes, in the order of their hashes.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_values()
    }
}

impl FromIterator<Vec<u8>> for Memory {
    fn from_iter<I: IntoIterator<Item = Vec<u8>>>(blobs: I) -> Self {
        let mut memory = Self::default();
        memory.extend(blobs);
        memory
    }
}
