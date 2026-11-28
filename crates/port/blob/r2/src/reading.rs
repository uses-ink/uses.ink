//! A blob being read from R2.

use blob::Chunks;
use futures_util::StreamExt as _;
use model::Hash;
use worker::ByteStream;

use crate::error::{Call, Error};

/// An object's body, read as it arrives and hashed as it goes. At the end its hash must be its name.
#[derive(Debug)]
pub struct Reading {
    stream: ByteStream,
    key: String,
    expected: Hash,
    hasher: blake3::Hasher,
    done: bool,
}

impl Reading {
    pub(crate) fn new(stream: ByteStream, key: String, expected: Hash) -> Self {
        Self { stream, key, expected, hasher: blake3::Hasher::new(), done: false }
    }
}

impl Chunks for Reading {
    type Error = Error;

    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, blob::Error<Error>> {
        if self.done {
            return Ok(None);
        }
        match self.stream.next().await {
            Some(Ok(bytes)) => {
                self.hasher.update(&bytes);
                Ok(Some(bytes))
            }
            Some(Err(source)) => Err(Error::Call { call: Call::Chunk, key: self.key.clone(), source }.into()),
            None => {
                // `done` is set only on a match: a blob that fails is refused again by every call
                let found = Hash::from(self.hasher.finalize());
                if found == self.expected {
                    self.done = true;
                    Ok(None)
                } else {
                    Err(blob::Error::Corrupt { expected: self.expected, found })
                }
            }
        }
    }
}
