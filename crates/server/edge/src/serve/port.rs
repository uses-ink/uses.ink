//! The two ports the edge reads through: which deploy a host serves, and the bytes behind a hash.
//!
//! Both are asynchronous, because a blob store and a row store are (R2, D1), and their futures
//! are `Send`, so a multi-threaded runtime can run them. A runtime that cannot send its futures
//! (workers-rs) wraps these traits in its own adapter; the edge's logic is the same.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

use bytes::Bytes;
use model::{Hash, Manifest, Route};

use crate::http::encoding::Encoding;
use crate::http::range::Span;
use crate::serve::hostname::Hostname;

/// Routes that moved: each old route and the route that took its place. A route the manifest
/// still serves is never moved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moved(BTreeMap<Route, Route>);

impl FromIterator<(Route, Route)> for Moved {
    fn from_iter<I: IntoIterator<Item = (Route, Route)>>(moves: I) -> Self {
        Self(moves.into_iter().collect())
    }
}

impl Moved {
    /// Where `route` went.
    pub fn to(&self, route: &Route) -> Option<&Route> {
        self.0.get(route)
    }
}

/// Pre-compressed variants of blobs: the blob of a file's bytes in a coding.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Variants(BTreeMap<(Hash, Encoding), Hash>);

impl FromIterator<(Hash, Encoding, Hash)> for Variants {
    fn from_iter<I: IntoIterator<Item = (Hash, Encoding, Hash)>>(variants: I) -> Self {
        Self(variants.into_iter().map(|(plain, encoding, variant)| ((plain, encoding), variant)).collect())
    }
}

impl Variants {
    /// The blob holding `plain` compressed as `encoding`, when the deploy has one.
    pub fn of(&self, plain: Hash, encoding: Encoding) -> Option<Hash> {
        self.0.get(&(plain, encoding)).copied()
    }

    /// The codings the deploy holds `plain` in.
    pub fn held(&self, plain: Hash) -> impl Iterator<Item = Encoding> + '_ {
        Encoding::ALL.into_iter().filter(move |encoding| self.0.contains_key(&(plain, *encoding)))
    }
}

/// What one host serves right now: the manifest, and what the manifest does not carry (v3 has no
/// redirects or variants, so adapters that read one give both empty).
#[derive(Clone, Debug, Default)]
pub struct Deploy {
    /// Every route, its blob, media type and headers.
    pub manifest: Manifest,
    /// Routes that moved.
    pub moved: Moved,
    /// Pre-compressed variants.
    pub variants: Variants,
}

/// A deploy and the scope its blobs are read in (a tenant, a folder).
#[derive(Debug)]
pub struct Resolved<S> {
    /// The deploy, shared with every other request for it.
    pub deploy: Arc<Deploy>,
    /// What [`Blobs::open`] reads the deploy's blobs under.
    pub scope: S,
}

/// Host to deploy: the site a name belongs to and its live deploy.
pub trait Sites: Send + Sync {
    /// What its blobs are scoped by.
    type Scope: Send + Sync;
    /// Why a lookup failed.
    type Error: Error + Send + Sync + 'static;

    /// The deploy `hostname` serves, or `None` when no site has that name.
    fn resolve(
        &self,
        hostname: &Hostname,
    ) -> impl Future<Output = Result<Option<Resolved<Self::Scope>>, Self::Error>> + Send;

    /// The notice to answer with when `hostname` serves nothing because its site is suspended.
    /// Asked only after [`Sites::resolve`] found nothing. A source with no suspensions says
    /// `None`, which is what this does.
    fn suspended(&self, hostname: &Hostname) -> impl Future<Output = Result<Option<Notice>, Self::Error>> + Send {
        let _ = hostname;
        async { Ok(None) }
    }
}

/// Why a host that serves nothing is not simply unknown: a site that is there and is suspended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Why the site is suspended, as the operator named it.
    pub reason: &'static str,
}

/// Why a blob could not be opened.
#[derive(Debug)]
#[non_exhaustive]
pub enum BlobError<E> {
    /// The store holds nothing under this hash.
    Missing {
        /// The hash.
        hash: Hash,
    },
    /// The store failed.
    Failed {
        /// The hash.
        hash: Hash,
        /// What the store said.
        source: E,
    },
}

impl<E> fmt::Display for BlobError<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { hash } => write!(out, "the blob {hash} is not in the store"),
            Self::Failed { hash, .. } => write!(out, "the blob {hash} could not be opened"),
        }
    }
}

impl<E: Error + 'static> Error for BlobError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Missing { .. } => None,
            Self::Failed { source, .. } => Some(source),
        }
    }
}

/// Bytes by hash, read in a scope.
pub trait Blobs: Send + Sync {
    /// What the hash is read under: the same type [`Sites`] resolves to.
    type Scope: Send + Sync;
    /// An opened blob.
    type Blob: Blob;
    /// Why the store failed.
    type Error: Error + Send + Sync + 'static;

    /// Opens the blob of `hash`, learning its length without reading it.
    fn open(
        &self,
        scope: &Self::Scope,
        hash: Hash,
    ) -> impl Future<Output = Result<Self::Blob, BlobError<Self::Error>>> + Send;
}

/// An opened blob: its length now, its bytes as a stream when asked.
pub trait Blob: Send + Sync {
    /// The stream of a span's bytes.
    type Body: Chunks;

    /// How many bytes it holds.
    fn length(&self) -> u64;

    /// The bytes of `span`, read lazily; `span` lies inside [`Blob::length`].
    fn read(self, span: Span) -> Self::Body;
}

/// A stream of bytes, a chunk at a time, so a large file is never held whole.
pub trait Chunks: Send {
    /// Why a read failed.
    type Error: Error + Send + Sync + 'static;

    /// The next chunk, `None` at the end.
    fn next(&mut self) -> impl Future<Output = Result<Option<Bytes>, Self::Error>> + Send;
}
