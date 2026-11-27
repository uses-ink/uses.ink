//! Blobs over a deploy's files: [`Files`].

use std::fs::Metadata;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use edge::{Blob, BlobError, Blobs, Chunks, Span};
use model::Hash;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::deploys::Placed;
use crate::error::Error;

/// How much of a file one chunk reads.
const CHUNK: u64 = 64 * 1024;

/// Reads a blob from the file the deploy's manifest lists for its hash.
#[derive(Clone, Copy, Debug, Default)]
pub struct Files;

/// An opened file: the handle, its path for the errors that name it, and its length when it was
/// opened. Every read goes through the handle, so a path replaced by a link after the open is
/// never followed.
#[derive(Debug)]
pub struct File {
    path: PathBuf,
    length: u64,
    handle: tokio::fs::File,
}

/// A span of a file, read a chunk at a time.
#[derive(Debug)]
pub struct Chunked {
    path: PathBuf,
    seek: Option<u64>,
    owed: u64,
    file: tokio::fs::File,
}

impl File {
    /// Opens `path` once and checks that the handle is the file `seen` described when the path
    /// was checked, so a swap between the check and the open is refused.
    async fn pin(path: PathBuf, seen: &Metadata, hash: Hash) -> Result<Self, BlobError<Error>> {
        let file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Err(BlobError::Missing { hash }),
            Err(source) => return Err(BlobError::Failed { hash, source: Error::Read { path, source } }),
        };
        let held = match file.metadata().await {
            Ok(held) => held,
            Err(source) => return Err(BlobError::Failed { hash, source: Error::Read { path, source } }),
        };
        if !held.is_file() || !Self::same(seen, &held) {
            return Err(BlobError::Failed { hash, source: Error::Swapped { path } });
        }
        Ok(Self { path, length: held.len(), handle: file })
    }

    /// Whether two descriptions are of one file.
    #[cfg(unix)]
    fn same(one: &Metadata, other: &Metadata) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        one.dev() == other.dev() && one.ino() == other.ino()
    }

    /// Whether two descriptions are of one file, as far as this system says.
    #[cfg(not(unix))]
    fn same(one: &Metadata, other: &Metadata) -> bool {
        one.len() == other.len() && one.modified().ok() == other.modified().ok()
    }
}

impl Blobs for Files {
    type Scope = Arc<Placed>;
    type Blob = File;
    type Error = Error;

    async fn open(&self, scope: &Arc<Placed>, hash: Hash) -> Result<File, BlobError<Error>> {
        let relative = scope.files.get(&hash).ok_or(BlobError::Missing { hash })?;
        let mut path = scope.root.clone();
        let mut parts = relative.as_str().split('/').peekable();
        let mut last = None;
        while let Some(part) = parts.next() {
            path.push(part);
            let meta = match tokio::fs::symlink_metadata(&path).await {
                Ok(meta) => meta,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                    return Err(BlobError::Missing { hash });
                }
                Err(source) => return Err(BlobError::Failed { hash, source: Error::Read { path, source } }),
            };
            if meta.is_symlink() {
                return Err(BlobError::Failed { hash, source: Error::Linked { path } });
            }
            let wanted = if parts.peek().is_some() { meta.is_dir() } else { meta.is_file() };
            if !wanted {
                return Err(BlobError::Failed { hash, source: Error::NotFile { path } });
            }
            last = Some(meta);
        }
        let seen = last.ok_or(BlobError::Missing { hash })?;
        File::pin(path, &seen, hash).await
    }
}

impl Blob for File {
    type Body = Chunked;

    fn length(&self) -> u64 {
        self.length
    }

    fn read(self, span: Span) -> Chunked {
        Chunked { path: self.path, seek: Some(span.start), owed: span.length, file: self.handle }
    }
}

impl Chunks for Chunked {
    type Error = Error;

    async fn next(&mut self) -> Result<Option<Bytes>, Error> {
        if self.owed == 0 {
            return Ok(None);
        }
        let read = |source| Error::Read { path: self.path.clone(), source };
        if let Some(start) = self.seek.take() {
            self.file.seek(SeekFrom::Start(start)).await.map_err(read)?;
        }
        let want = usize::try_from(self.owed.min(CHUNK)).unwrap_or(usize::MAX);
        let mut chunk = vec![0; want];
        let got = self.file.read(&mut chunk).await.map_err(read)?;
        if got == 0 {
            return Err(Error::Short { path: self.path.clone(), owed: self.owed });
        }
        chunk.truncate(got);
        self.owed -= got as u64;
        Ok(Some(Bytes::from(chunk)))
    }
}
