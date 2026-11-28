//! A blob being written to R2: hashed as it is taken, put whole while it fits one part, cut into
//! a multipart upload when it does not.

use std::mem;

use blob::Stored;
use model::{Bytes, Hash};
use worker::{Bucket, MultipartUpload, UploadedPart};

use crate::error::{Call, Error, Reason};
use crate::limits::Limits;

/// A multipart upload and the parts it has so far.
struct Open<'a> {
    key: &'a str,
    upload: MultipartUpload,
    parts: Vec<UploadedPart>,
}

impl<'a> Open<'a> {
    /// Starts an upload of `key`.
    async fn begin(bucket: &Bucket, key: &'a str) -> Result<Self, Error> {
        let upload = bucket.create_multipart_upload(key).execute().await.map_err(|source| Error::Call {
            call: Call::Begin,
            key: key.to_owned(),
            source,
        })?;
        Ok(Self { key, upload, parts: Vec::new() })
    }

    /// Uploads `bytes` as the next part.
    async fn send(&mut self, bytes: Vec<u8>) -> Result<(), Error> {
        if self.parts.len() >= Limits::PARTS {
            return Err(Error::Parts { key: self.key.to_owned(), most: Limits::PARTS });
        }
        let number = u16::try_from(self.parts.len() + 1).unwrap_or(u16::MAX);
        let part = self.upload.upload_part(number, bytes).await.map_err(|source| Error::Call {
            call: Call::Part(number),
            key: self.key.to_owned(),
            source,
        })?;
        self.parts.push(part);
        Ok(())
    }

    /// Makes the object of the parts sent.
    async fn finish(self) -> Result<(), Error> {
        let key = self.key.to_owned();
        self.upload.complete(self.parts).await.map(drop).map_err(|source| Error::Call {
            call: Call::Complete,
            key,
            source,
        })
    }

    /// Gives the upload up, so R2 keeps none of its parts: what the abort failed with, if it did.
    async fn abort(self) -> Option<worker::Error> {
        self.upload.abort().await.err()
    }
}

/// One write.
pub(crate) struct Writing<'a> {
    pub(crate) bucket: &'a Bucket,
    pub(crate) key: &'a str,
    pub(crate) limits: Limits,
}

/// What a pass over the bytes found: their hash and how many there were.
#[derive(Clone, Copy, Debug)]
struct Taken {
    found: Hash,
    size: u64,
}

impl<'a> Writing<'a> {
    /// Checks the bytes against `hash` and stores them unless `held`: the blob is already there, so
    /// only its bytes are read.
    pub(crate) async fn run(
        &self,
        hash: Hash,
        held: bool,
        chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, blob::Error<Error>> {
        if held {
            let taken = Self::count(chunks);
            return if taken.found == hash {
                Ok(Stored { size: Bytes(taken.size), created: false })
            } else {
                Err(blob::Error::Mismatch { expected: hash, found: taken.found })
            };
        }
        let mut hasher = blake3::Hasher::new();
        let mut buffer: Vec<u8> = Vec::new();
        let mut open: Option<Open<'a>> = None;
        let mut size = 0_u64;
        for chunk in chunks {
            let chunk = chunk.as_ref();
            hasher.update(chunk);
            size += chunk.len() as u64;
            buffer.extend_from_slice(chunk);
            // a blob of exactly one part is put whole: only a longer one is cut
            while buffer.len() > self.limits.part() {
                let rest = buffer.split_off(self.limits.part());
                let full = mem::replace(&mut buffer, rest);
                if let Err(error) = self.cut(&mut open, full).await {
                    return Err(Self::undone(open, error).await.into());
                }
            }
        }
        let found = Hash::from(hasher.finalize());
        if found != hash {
            return Err(Self::refused(open, hash, found).await);
        }
        self.finish(open, buffer).await?;
        Ok(Stored { size: Bytes(size), created: true })
    }

    /// The refusal of bytes that hash to `found`, with their upload, if one was begun, given up.
    async fn refused(open: Option<Open<'a>>, hash: Hash, found: Hash) -> blob::Error<Error> {
        if let Some(open) = open {
            let key = open.key.to_owned();
            if let Some(source) = open.abort().await {
                return Error::Abandoned { key, source, reason: Reason::Mismatch { expected: hash, found } }.into();
            }
        }
        blob::Error::Mismatch { expected: hash, found }
    }

    /// Hashes `chunks` without keeping them.
    fn count(chunks: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Taken {
        let mut hasher = blake3::Hasher::new();
        let mut size = 0_u64;
        for chunk in chunks {
            hasher.update(chunk.as_ref());
            size += chunk.as_ref().len() as u64;
        }
        Taken { found: Hash::from(hasher.finalize()), size }
    }

    /// Sends a full part, beginning the upload with the first.
    async fn cut(&self, open: &mut Option<Open<'a>>, part: Vec<u8>) -> Result<(), Error> {
        if open.is_none() {
            *open = Some(Open::begin(self.bucket, self.key).await?);
        }
        match open {
            Some(upload) => upload.send(part).await,
            None => Ok(()),
        }
    }

    /// Ends the write: the last part and the completion, or the whole blob in one put.
    async fn finish(&self, open: Option<Open<'a>>, rest: Vec<u8>) -> Result<(), Error> {
        let Some(mut open) = open else {
            return self.bucket.put(self.key, rest).execute().await.map(drop).map_err(|source| Error::Call {
                call: Call::Put,
                key: self.key.to_owned(),
                source,
            });
        };
        if !rest.is_empty()
            && let Err(error) = open.send(rest).await
        {
            return Err(Self::undone(Some(open), error).await);
        }
        open.finish().await
    }

    /// The failure of a part, with its upload given up.
    async fn undone(open: Option<Open<'a>>, error: Error) -> Error {
        let Some(open) = open else { return error };
        let key = open.key.to_owned();
        match open.abort().await {
            None => error,
            Some(source) => Error::Abandoned { key, source, reason: Reason::Failed(Box::new(error)) },
        }
    }
}
