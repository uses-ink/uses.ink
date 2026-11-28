//! The adapter.

use logging::Field as _;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use blob::{Blob, Deleted, Listed, Stored, Tenant};
use model::{Bytes, Hash};

use crate::{Error, Reading};

type Failure = blob::Error<Error>;

/// A blob store in a folder.
#[derive(Debug)]
pub struct Directory {
    root: PathBuf,
}

impl Directory {
    /// Opens the store at `root`, making the folder and its `t/` and `tmp/` when they are missing.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        for folder in [root.join("t"), root.join("tmp")] {
            fs::create_dir_all(&folder).map_err(|source| Error::Create { path: folder, source })?;
        }
        Ok(Self { root })
    }

    /// The folder a tenant's blobs are in: one file a blob, named by its hash.
    pub fn folder(&self, tenant: &Tenant) -> PathBuf {
        self.root.join("t").join(tenant.as_str()).join("b")
    }

    /// The file of one blob.
    fn file(&self, tenant: &Tenant, hash: Hash) -> PathBuf {
        self.folder(tenant).join(hash.to_string())
    }

    /// A fresh name in `tmp/` for a write in progress. The count is the process's, not the
    /// store's, so two stores on one root never draw the same name.
    fn partial(&self, hash: Hash) -> PathBuf {
        static WRITES: AtomicU64 = AtomicU64::new(0);
        let n = WRITES.fetch_add(1, Ordering::Relaxed);
        self.root.join("tmp").join(format!("{}-{n}-{hash}.part", std::process::id()))
    }

    /// The metadata of a blob's file, none if it is missing; a symlink or folder is a stray.
    fn find(path: &Path) -> Result<Option<fs::Metadata>, Failure> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => Ok(Some(meta)),
            Ok(_) => Err(Error::Stray { path: path.to_path_buf() }.into()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::Read { path: path.to_path_buf(), source }.into()),
        }
    }

    /// Writes the chunks to `partial` and returns what they hash to and their size.
    fn fill(partial: &Path, chunks: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Result<(Hash, u64), Error> {
        let write = |source| Error::Write { path: partial.to_path_buf(), source };
        let mut file = OpenOptions::new().write(true).create_new(true).open(partial).map_err(write)?;
        let mut hasher = blake3::Hasher::new();
        let mut size = 0_u64;
        for chunk in chunks {
            let chunk = chunk.as_ref();
            file.write_all(chunk).map_err(write)?;
            hasher.update(chunk);
            size += chunk.len() as u64;
        }
        file.sync_all().map_err(write)?;
        Ok((Hash::from(hasher.finalize()), size))
    }

    /// Removes a half-written file, keeping `cause` as the failure; if the removal fails too,
    /// says so.
    fn discard(partial: &Path, cause: Failure) -> Failure {
        match fs::remove_file(partial) {
            Ok(()) => cause,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => cause,
            Err(source) => match cause {
                blob::Error::Backend { source: cause } => {
                    Error::Discard { path: partial.to_path_buf(), source, cause: Box::new(cause) }.into()
                }
                other => other,
            },
        }
    }

    /// Moves the finished file into place and syncs the folder, so the rename is durable.
    fn place(partial: &Path, target: &Path) -> Result<(), Error> {
        let write = |path: &Path| {
            let path = path.to_path_buf();
            move |source| Error::Write { path, source }
        };
        let folder = target.parent().unwrap_or(target);
        fs::create_dir_all(folder).map_err(|source| Error::Create { path: folder.to_path_buf(), source })?;
        fs::rename(partial, target).map_err(write(target))?;
        File::open(folder).and_then(|dir| dir.sync_all()).map_err(write(folder))
    }
}

impl From<Error> for Failure {
    fn from(source: Error) -> Self {
        Self::Backend { source }
    }
}

impl Blob for Directory {
    type Error = Error;
    type Read = Reading;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn put(
        &self,
        tenant: &Tenant,
        hash: Hash,
        chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, Failure> {
        // the body has no await point, so the span can be held across it
        let span = logging::blob!(logging::Io::Write, hash.as_bytes());
        let _entered = span.clone().entered();
        let target = self.file(tenant, hash);
        let partial = self.partial(hash);
        let (found, size) = Self::fill(&partial, chunks).map_err(|error| Self::discard(&partial, error.into()))?;
        if found != hash {
            return Err(Self::discard(&partial, blob::Error::Mismatch { expected: hash, found }));
        }
        let held = Self::find(&target).map_err(|error| Self::discard(&partial, error))?.is_some();
        if held {
            fs::remove_file(&partial).map_err(|source| Error::Remove { path: partial, source })?;
        } else {
            Self::place(&partial, &target).map_err(|error| Self::discard(&partial, error.into()))?;
        }
        span.record("bytes", logging::Size(size).field());
        logging::debug!(name: logging::event::BLOB_IO, created = !held);
        Ok(Stored { size: Bytes(size), created: !held })
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn get(&self, tenant: &Tenant, hash: Hash) -> Result<Option<Reading>, Failure> {
        let _entered = logging::blob!(logging::Io::Read, hash.as_bytes()).entered();
        let path = self.file(tenant, hash);
        if Self::find(&path)?.is_none() {
            logging::debug!(name: logging::event::BLOB_IO, found = false);
            return Ok(None);
        }
        match File::open(&path) {
            Ok(file) => Ok(Some(Reading::new(file, path, hash))),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::Read { path, source }.into()),
        }
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn stat(&self, tenant: &Tenant, hash: Hash) -> Result<Option<Bytes>, Failure> {
        let _entered = logging::blob!(logging::Io::Probe, hash.as_bytes()).entered();
        Ok(Self::find(&self.file(tenant, hash))?.map(|meta| Bytes(meta.len())))
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn list(&self, tenant: &Tenant) -> Result<Vec<Listed>, Failure> {
        let folder = self.folder(tenant);
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(Error::Read { path: folder, source }.into()),
        };
        let mut listed = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| Error::Read { path: folder.clone(), source })?;
            let path = entry.path();
            let hash: Hash = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse().ok())
                .ok_or(Error::Stray { path: path.clone() })?;
            let meta = Self::find(&path)?.ok_or(Error::Stray { path })?;
            listed.push(Listed { hash, size: Bytes(meta.len()) });
        }
        listed.sort();
        Ok(listed)
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn delete(&self, tenant: &Tenant, hash: Hash) -> Result<Deleted, Failure> {
        let _entered = logging::blob!(logging::Io::Remove, hash.as_bytes()).entered();
        let path = self.file(tenant, hash);
        let Some(meta) = Self::find(&path)? else { return Ok(Deleted::Absent) };
        match fs::remove_file(&path) {
            Ok(()) => Ok(Deleted::Removed(Bytes(meta.len()))),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Deleted::Absent),
            Err(source) => Err(Error::Remove { path, source }.into()),
        }
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use super::*;

    #[test]
    fn two_stores_on_one_root_draw_different_partials() {
        let folder = tempfile::tempdir().unwrap();
        let (one, two) = (Directory::open(folder.path()).unwrap(), Directory::open(folder.path()).unwrap());
        let hash = Hash::of(b"same");
        assert_ne!(one.partial(hash), two.partial(hash));
    }
}
