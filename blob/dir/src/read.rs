//! A blob being read from disk.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use blob::Chunks;
use model::Hash;

use crate::Error;

/// A blob file read in chunks and hashed as it goes. At the end its hash must be its name.
#[derive(Debug)]
pub struct Reading {
    file: File,
    path: PathBuf,
    expected: Hash,
    hasher: blake3::Hasher,
    done: bool,
}

impl Reading {
    /// How much one chunk holds at most.
    pub(crate) const CHUNK: usize = 64 * 1024;

    pub(crate) fn new(file: File, path: PathBuf, expected: Hash) -> Self {
        Self { file, path, expected, hasher: blake3::Hasher::new(), done: false }
    }
}

impl Chunks for Reading {
    type Error = Error;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, blob::Error<Error>> {
        if self.done {
            return Ok(None);
        }
        let mut buffer = vec![0_u8; Self::CHUNK];
        let read = self
            .file
            .read(&mut buffer)
            .map_err(|source| blob::Error::Backend { source: Error::Read { path: self.path.clone(), source } })?;
        if read == 0 {
            self.done = true;
            let found = Hash::from(self.hasher.finalize());
            return if found == self.expected {
                Ok(None)
            } else {
                Err(blob::Error::Corrupt { expected: self.expected, found })
            };
        }
        buffer.truncate(read);
        self.hasher.update(&buffer);
        Ok(Some(buffer))
    }
}
