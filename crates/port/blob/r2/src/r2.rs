//! The adapter.

use blob::{Blob, Deleted, Listed, Stored, Tenant};
use logging::Field as _;
use model::{Bytes, Hash};
use worker::{Bucket, Object};

use crate::error::{Call, Error, Key};
use crate::limits::Limits;
use crate::reading::Reading;
use crate::writing::Writing;

type Failure = blob::Error<Error>;

/// A blob store in an R2 bucket: a binding of a Worker. An object is `<tenant>/<hash>`.
#[derive(Debug)]
pub struct R2 {
    bucket: Bucket,
    limits: Limits,
}

impl R2 {
    /// The bucket a Worker's R2 binding gives, from `env.bucket(..)`, with R2's own limits.
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket, limits: Limits::default() }
    }

    /// The same bucket worked to other limits.
    pub fn with(bucket: Bucket, limits: Limits) -> Self {
        Self { bucket, limits }
    }

    /// The metadata of the object at `key`, if there is one.
    async fn head(&self, key: &str) -> Result<Option<Object>, Error> {
        self.bucket.head(key).await.map_err(|source| Error::Call { call: Call::Head, key: key.to_owned(), source })
    }

    /// Stores the bytes, in the span of the write.
    async fn store(
        &self,
        key: &str,
        hash: Hash,
        chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, Failure> {
        let held = self.head(key).await?.is_some();
        Writing { bucket: &self.bucket, key, limits: self.limits }.run(hash, held, chunks).await
    }
}

impl Blob for R2 {
    type Error = Error;
    type Read = Reading;

    async fn put(
        &self,
        tenant: &Tenant,
        hash: Hash,
        chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, Failure> {
        let span = logging::blob!(logging::Io::Write, hash.as_bytes());
        let key = Key { tenant, hash }.to_string();
        let stored = self.store(&key, hash, chunks).await;
        span.in_scope(|| match &stored {
            Ok(done) => {
                span.record("bytes", logging::Size(done.size.0).field());
                logging::debug!(name: logging::event::BLOB_IO, created = done.created);
            }
            Err(error) => logging::warn!(name: logging::event::BLOB_IO, error = logging::Failure(error).field()),
        });
        stored
    }

    async fn get(&self, tenant: &Tenant, hash: Hash) -> Result<Option<Reading>, Failure> {
        let span = logging::blob!(logging::Io::Read, hash.as_bytes());
        let key = Key { tenant, hash }.to_string();
        let call = |source| Error::Call { call: Call::Get, key: key.clone(), source };
        let found = self.bucket.get(&key).execute().await.map_err(call)?;
        span.in_scope(|| logging::debug!(name: logging::event::BLOB_IO, found = found.is_some()));
        let Some(object) = found else { return Ok(None) };
        let body = object.body().ok_or_else(|| Error::Bodiless { key: key.clone() })?;
        let stream = body.stream().map_err(call)?;
        Ok(Some(Reading::new(stream, key, hash)))
    }

    async fn stat(&self, tenant: &Tenant, hash: Hash) -> Result<Option<Bytes>, Failure> {
        let key = Key { tenant, hash }.to_string();
        Ok(self.head(&key).await?.map(|object| Bytes(object.size())))
    }

    async fn list(&self, tenant: &Tenant) -> Result<Vec<Listed>, Failure> {
        let prefix = format!("{tenant}/");
        let mut listed = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut page = self.bucket.list().prefix(prefix.clone()).limit(self.limits.page());
            if let Some(cursor) = cursor.take() {
                page = page.cursor(cursor);
            }
            let objects =
                page.execute().await.map_err(|source| Error::Call { call: Call::List, key: prefix.clone(), source })?;
            for object in objects.objects() {
                let key = object.key();
                let hash: Hash = key
                    .strip_prefix(&prefix)
                    .and_then(|name| name.parse().ok())
                    .ok_or_else(|| Error::Stray { key: key.clone() })?;
                listed.push(Listed { hash, size: Bytes(object.size()) });
            }
            if !objects.truncated() {
                break;
            }
            cursor = Some(objects.cursor().ok_or_else(|| Error::Unfinished { prefix: prefix.clone() })?);
        }
        listed.sort();
        Ok(listed)
    }

    async fn delete(&self, tenant: &Tenant, hash: Hash) -> Result<Deleted, Failure> {
        let key = Key { tenant, hash }.to_string();
        let Some(object) = self.head(&key).await? else { return Ok(Deleted::Absent) };
        let size = Bytes(object.size());
        self.bucket.delete(&key).await.map_err(|source| Error::Call { call: Call::Delete, key, source })?;
        Ok(Deleted::Removed(size))
    }
}
