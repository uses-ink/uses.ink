//! The blob port of the edge over the blob port of the control plane.

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use blob::{Blob as Store, Chunks as Read, Tenant};
use bytes::Bytes;
use edge::{Blob, BlobError, Blobs, Chunks, Span};
use edge_control::Chain;
use model::Hash;
use send_wrapper::SendWrapper;

/// [`Blobs`] over any blob store of the control plane: a blob is `stat`ted for its length when it
/// is opened and read from its start when it is streamed, so a store that verifies a read (R2's
/// does) verifies a whole file. A span skips the bytes before it; a store is read from its start
/// whatever the span, which keeps every byte served one the hash vouched for, and costs the skipped
/// bytes. The store is held in a wrapper that is `Send`, for a binding is not.
#[derive(Debug)]
pub struct Stored<B> {
    store: Arc<SendWrapper<B>>,
}

impl<B> Clone for Stored<B> {
    fn clone(&self) -> Self {
        Self { store: Arc::clone(&self.store) }
    }
}

impl<B: Store> Stored<B> {
    /// Blobs read from `store`.
    pub fn new(store: B) -> Self {
        Self { store: Arc::new(SendWrapper::new(store)) }
    }
}

/// A blob that ended before the bytes asked for, or was gone when its bytes were asked for.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Cut {
    /// It was held when it was opened, and was not when it was read.
    Vanished {
        /// The blob.
        hash: Hash,
    },
    /// Its bytes ran out before the span did.
    Short {
        /// The blob.
        hash: Hash,
        /// How many bytes of the span were still to come.
        missing: u64,
    },
}

impl fmt::Display for Cut {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vanished { hash } => write!(out, "the blob {hash} was held when it was opened and is gone"),
            Self::Short { hash, missing } => {
                write!(out, "the blob {hash} ended {missing} bytes before the span it was asked for")
            }
        }
    }
}

impl std::error::Error for Cut {}

/// An opened blob: its length, and where to read it.
#[derive(Debug)]
pub struct Opened<B> {
    store: Arc<SendWrapper<B>>,
    tenant: Tenant,
    hash: Hash,
    length: u64,
}

impl<B: Store + 'static> Blobs for Stored<B> {
    type Scope = Tenant;
    type Blob = Opened<B>;
    type Error = Chain;

    fn open(&self, scope: &Tenant, hash: Hash) -> impl Future<Output = Result<Opened<B>, BlobError<Chain>>> + Send {
        let (store, tenant) = (Arc::clone(&self.store), scope.clone());
        SendWrapper::new(async move {
            match store.stat(&tenant, hash).await {
                Ok(Some(size)) => Ok(Opened { store, tenant, hash, length: size.0 }),
                Ok(None) => Err(BlobError::Missing { hash }),
                Err(error) => Err(BlobError::Failed { hash, source: Chain::of(&error) }),
            }
        })
    }
}

impl<B: Store + 'static> Blob for Opened<B> {
    type Body = Reading<B>;

    fn length(&self) -> u64 {
        self.length
    }

    fn read(self, span: Span) -> Reading<B> {
        let to_end = span.start.saturating_add(span.length) >= self.length;
        let inner = Inner { source: self, skip: span.start, left: span.length, to_end, read: None };
        Reading { inner: SendWrapper::new(inner) }
    }
}

/// What reading a span keeps between chunks.
struct Inner<B: Store> {
    source: Opened<B>,
    skip: u64,
    left: u64,
    to_end: bool,
    read: Option<B::Read>,
}

/// The bytes of a span, a chunk at a time.
pub struct Reading<B: Store> {
    inner: SendWrapper<Inner<B>>,
}

impl<B: Store> fmt::Debug for Reading<B> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Reading(..)")
    }
}

impl<B: Store> Inner<B> {
    /// The next chunk of the span, or none once the span is read. A span that reaches the end of
    /// the blob reads the store's own end too, which is where a verifying store says the bytes
    /// were not the hash.
    async fn pull(&mut self) -> Result<Option<Bytes>, Chain> {
        let (tenant, hash) = (self.source.tenant.clone(), self.source.hash);
        if self.left == 0 && self.read.is_none() {
            return Ok(None);
        }
        if self.read.is_none() {
            let opened = self.source.store.get(&tenant, hash).await.map_err(|error| Chain::of(&error))?;
            self.read = Some(opened.ok_or_else(|| Chain::of(&Cut::Vanished { hash }))?);
        }
        loop {
            let Some(read) = self.read.as_mut() else { return Ok(None) };
            let chunk = read.chunk().await.map_err(|error| Chain::of(&error))?;
            let Some(mut chunk) = chunk else {
                self.read = None;
                return match self.left {
                    0 => Ok(None),
                    missing => Err(Chain::of(&Cut::Short { hash, missing })),
                };
            };
            if self.left == 0 {
                // the span is read; the rest of the blob is read only to be checked
                continue;
            }
            let length = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
            let skipped = self.skip.min(length);
            self.skip -= skipped;
            if skipped == length {
                continue;
            }
            chunk = chunk.split_off(usize::try_from(skipped).unwrap_or(usize::MAX));
            let taken = u64::try_from(chunk.len()).unwrap_or(u64::MAX).min(self.left);
            chunk.truncate(usize::try_from(taken).unwrap_or(usize::MAX));
            self.left -= taken;
            if self.left == 0 && !self.to_end {
                self.read = None;
            }
            return Ok(Some(Bytes::from(chunk)));
        }
    }
}

impl<B: Store + 'static> Chunks for Reading<B> {
    type Error = Chain;

    fn next(&mut self) -> impl Future<Output = Result<Option<Bytes>, Chain>> + Send {
        SendWrapper::new(self.inner.pull())
    }
}
