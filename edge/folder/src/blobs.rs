//! Blobs over a deploy's files: [`Files`].

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

/// An opened file: its path and its length when it was opened.
#[derive(Debug)]
pub struct File {
    path: PathBuf,
    length: u64,
}

/// A span of a file, read a chunk at a time.
#[derive(Debug)]
pub struct Chunked {
    path: PathBuf,
    next: u64,
    owed: u64,
    file: Option<tokio::fs::File>,
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
            last = Some(meta.len());
        }
        let length = last.ok_or(BlobError::Missing { hash })?;
        Ok(File { path, length })
    }
}

impl Blob for File {
    type Body = Chunked;

    fn length(&self) -> u64 {
        self.length
    }

    fn read(self, span: Span) -> Chunked {
        Chunked { path: self.path, next: span.start, owed: span.length, file: None }
    }
}

impl Chunks for Chunked {
    type Error = Error;

    async fn next(&mut self) -> Result<Option<Bytes>, Error> {
        if self.owed == 0 {
            return Ok(None);
        }
        let read = |source| Error::Read { path: self.path.clone(), source };
        let file = if let Some(file) = self.file.as_mut() {
            file
        } else {
            let mut file = tokio::fs::File::open(&self.path).await.map_err(read)?;
            file.seek(SeekFrom::Start(self.next)).await.map_err(read)?;
            self.file.insert(file)
        };
        let want = usize::try_from(self.owed.min(CHUNK)).unwrap_or(usize::MAX);
        let mut chunk = vec![0; want];
        let got = file.read(&mut chunk).await.map_err(read)?;
        if got == 0 {
            return Err(Error::Short { path: self.path.clone(), owed: self.owed });
        }
        chunk.truncate(got);
        let got = got as u64;
        self.next += got;
        self.owed -= got;
        Ok(Some(Bytes::from(chunk)))
    }
}
