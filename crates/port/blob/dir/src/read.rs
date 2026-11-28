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
            // `done` is set only on a match: a blob that fails is refused again by every call
            let found = Hash::from(self.hasher.finalize());
            return if found == self.expected {
                self.done = true;
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

#[cfg(test)]
mod test {
    use blob::Chunks;
    use blob_conformance::pollster::block_on;

    use super::*;

    #[test]
    fn a_corrupt_blob_is_refused_by_every_call_after_its_bytes() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("blob");
        std::fs::write(&path, b"not what its name says").unwrap();
        let mut reading = Reading::new(File::open(&path).unwrap(), path, Hash::of(b"what it was named"));
        assert!(block_on(reading.chunk()).unwrap().is_some());
        for _ in 0..3 {
            assert!(matches!(block_on(reading.chunk()), Err(blob::Error::Corrupt { .. })));
        }
    }
}
