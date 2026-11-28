//! The port itself.

use model::{Bytes, Hash};

use crate::{Error, Tenant};

/// What a write did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stored {
    /// The blob's size.
    pub size: Bytes,
    /// Whether this write made the blob, or it was already held (same bytes, same key).
    pub created: bool,
}

/// What a delete did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Deleted {
    /// The blob existed and is gone.
    Removed(Bytes),
    /// There was nothing to delete.
    Absent,
}

/// One blob a listing names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Listed {
    /// Its key.
    pub hash: Hash,
    /// Its size.
    pub size: Bytes,
}

/// A blob being read, chunk by chunk, so a large one is never held whole by the port.
#[expect(async_fn_in_trait, reason = "R2 futures are not Send on Workers, so no Send bound can be promised")]
pub trait Chunks {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;

    /// The next chunk, or none at the end. An adapter that verifies as it reads reports
    /// [`Error::Corrupt`] here, from the last call.
    async fn chunk(&mut self) -> Result<Option<Vec<u8>>, Error<Self::Error>>;

    /// Every remaining chunk, joined.
    async fn all(mut self) -> Result<Vec<u8>, Error<Self::Error>>
    where
        Self: Sized,
    {
        let mut bytes = Vec::new();
        while let Some(chunk) = self.chunk().await? {
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

/// Content-addressed bytes per tenant. Futures are not required to be `Send`: R2's are not.
#[expect(async_fn_in_trait, reason = "R2 futures are not Send on Workers, so no Send bound can be promised")]
pub trait Blob {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;
    /// A blob being read.
    type Read: Chunks<Error = Self::Error>;

    /// Stores the bytes `chunks` yield, in order, as the blob `hash` of `tenant`. The bytes are
    /// hashed as they are taken, and a different hash is [`Error::Mismatch`] with nothing stored.
    /// Storing a blob already held changes nothing, but its bytes are still checked.
    async fn put(
        &self,
        tenant: &Tenant,
        hash: Hash,
        chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, Error<Self::Error>>;

    /// The blob `hash` of `tenant`, or none if it is not held.
    async fn get(&self, tenant: &Tenant, hash: Hash) -> Result<Option<Self::Read>, Error<Self::Error>>;

    /// The size of the blob, or none if it is not held.
    async fn stat(&self, tenant: &Tenant, hash: Hash) -> Result<Option<Bytes>, Error<Self::Error>>;

    /// Whether the blob is held.
    async fn exists(&self, tenant: &Tenant, hash: Hash) -> Result<bool, Error<Self::Error>> {
        Ok(self.stat(tenant, hash).await?.is_some())
    }

    /// Every blob of `tenant`, by hash.
    async fn list(&self, tenant: &Tenant) -> Result<Vec<Listed>, Error<Self::Error>>;

    /// Removes the blob.
    async fn delete(&self, tenant: &Tenant, hash: Hash) -> Result<Deleted, Error<Self::Error>>;
}
