//! Work kept between builds. A build that makes something expensive (an image's copies, a
//! render unit: a page's HTML) puts it under a [`Key`] naming everything it
//! was made from; a later build with the same key takes it instead of making it again. Values
//! are bytes, so a cache can hold them by their hash and check them when they come back.
//!
//! The engine only names the port: where the values live (a folder, a bucket) is the host's.
//! A cache never changes what a build gives: the same key always names the same values, so a
//! build gives the same deploy, byte for byte, with a cache, a cold one, or none.

use std::fmt;

use model::Hash;

/// Values kept between builds, by key.
///
/// A miss says nothing of why (never kept, evicted, found corrupt): a cache that drops a value
/// says so itself, through its host's notices.
pub trait Cache: Send + Sync {
    /// The values kept under `key`, in the order they were put, or `None`.
    fn get(&self, key: &Key) -> Option<Vec<Vec<u8>>>;

    /// Keeps `values` under `key`, in this order, in place of anything kept there before.
    fn put(&self, key: &Key, values: &[&[u8]]);

    /// Drops what is kept under `key`: the values [`Cache::get`] gave for it could not be used.
    fn reject(&self, key: &Key);
}

impl Cache for () {
    /// Nothing is kept.
    fn get(&self, _key: &Key) -> Option<Vec<Vec<u8>>> {
        None
    }

    fn put(&self, _key: &Key, _values: &[&[u8]]) {}

    fn reject(&self, _key: &Key) {}
}

impl<C: Cache + ?Sized> Cache for &C {
    fn get(&self, key: &Key) -> Option<Vec<Vec<u8>>> {
        C::get(self, key)
    }

    fn put(&self, key: &Key, values: &[&[u8]]) {
        C::put(self, key, values);
    }

    fn reject(&self, key: &Key) {
        C::reject(self, key);
    }
}

/// What a cache keeps one piece of work under: its kind, and the hash of everything it was
/// made from. Two kinds never share a key, whatever their hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub struct Key {
    /// What the work is.
    pub kind: Kind,
    /// The hash of everything the work was made from: inputs, settings, versions.
    pub digest: Hash,
}

impl Key {
    /// The work of `kind` made from what hashes to `digest`.
    pub const fn new(kind: Kind, digest: Hash) -> Self {
        Self { kind, digest }
    }
}

impl fmt::Display for Key {
    /// `image/<digest>`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}/{}", self.kind, self.digest)
    }
}

/// The kinds of work a cache keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Kind {
    /// One image's copies and placeholder, as the host's image work made them.
    Image,
    /// One page as the theme rendered it: its HTML (decision 0048).
    Page,
}

impl Kind {
    /// Its name: one lower-case word, the same on every surface.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Page => "page",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}
