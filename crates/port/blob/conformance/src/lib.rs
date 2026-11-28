//! The conformance suite of the blob port: each case is a type with a `check` over any
//! [`Blob`], and [`suite!`] turns every case into a test.
//!
//! ```ignore
//! blob_conformance::suite!(|| MyAdapter::in_a_fresh_place());
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use blob::{Blob, Chunks, Deleted, Error, Listed, Tenant};
use model::{Bytes, Hash};

#[doc(hidden)]
pub use pollster;

/// Values the cases share.
struct Samples;

impl Samples {
    /// A tenant of the suite's.
    fn tenant(name: &str) -> Tenant {
        name.parse().unwrap()
    }
}

/// What goes in comes out.
#[derive(Debug, Clone, Copy)]
pub struct RoundTrip;

impl RoundTrip {
    /// Chunks written are one blob read back, and writing it again changes nothing.
    pub async fn check<B: Blob>(blob: &B) {
        let t = Samples::tenant("site-a");
        let bytes = b"hello, blob".to_vec();
        let hash = Hash::of(&bytes);
        assert!(blob.get(&t, hash).await.unwrap().is_none());
        assert_eq!(blob.stat(&t, hash).await.unwrap(), None);
        assert!(!blob.exists(&t, hash).await.unwrap());
        let (head, tail) = bytes.split_at(5);
        let stored = blob.put(&t, hash, [head, tail]).await.unwrap();
        assert_eq!((stored.size, stored.created), (Bytes(11), true));
        let again = blob.put(&t, hash, [bytes.as_slice()]).await.unwrap();
        assert_eq!((again.size, again.created), (Bytes(11), false));
        assert_eq!(blob.get(&t, hash).await.unwrap().unwrap().all().await.unwrap(), bytes);
        assert_eq!(blob.stat(&t, hash).await.unwrap(), Some(Bytes(11)));
        assert!(blob.exists(&t, hash).await.unwrap());
    }
}

/// The empty blob and a large one.
#[derive(Debug, Clone, Copy)]
pub struct Sizes;

impl Sizes {
    /// Zero bytes and a few hundred kilobytes in odd chunks both round-trip.
    pub async fn check<B: Blob>(blob: &B) {
        let t = Samples::tenant("site-a");
        let empty = Hash::of([]);
        blob.put(&t, empty, Vec::<Vec<u8>>::new()).await.unwrap();
        assert_eq!(blob.get(&t, empty).await.unwrap().unwrap().all().await.unwrap(), Vec::<u8>::new());
        let big: Vec<u8> = (0..300_000_u32).map(|n| u8::try_from(n % 253).unwrap()).collect();
        let hash = Hash::of(&big);
        let stored = blob.put(&t, hash, big.chunks(7_001)).await.unwrap();
        assert_eq!(stored.size, Bytes(300_000));
        let mut read = blob.get(&t, hash).await.unwrap().unwrap();
        let mut joined = Vec::new();
        while let Some(chunk) = read.chunk().await.unwrap() {
            joined.extend_from_slice(&chunk);
        }
        assert!(joined == big, "the bytes read are not the bytes written");
    }
}

/// A write is checked against the hash it names.
#[derive(Debug, Clone, Copy)]
pub struct Verified;

impl Verified {
    /// Other bytes than the named hash's are refused and leave nothing behind, even where the
    /// blob is already held.
    pub async fn check<B: Blob>(blob: &B) {
        let t = Samples::tenant("site-a");
        let hash = Hash::of(b"right");
        let refused = blob.put(&t, hash, [b"wrong"]).await.unwrap_err();
        assert!(
            matches!(refused, Error::Mismatch { expected, found } if expected == hash && found == Hash::of(b"wrong")),
            "{refused}"
        );
        assert!(!blob.exists(&t, hash).await.unwrap());
        assert!(!blob.exists(&t, Hash::of(b"wrong")).await.unwrap());
        assert!(blob.list(&t).await.unwrap().is_empty());
        blob.put(&t, hash, [b"right"]).await.unwrap();
        let held = blob.put(&t, hash, [b"wrong"]).await.unwrap_err();
        assert!(matches!(held, Error::Mismatch { .. }));
        assert_eq!(blob.get(&t, hash).await.unwrap().unwrap().all().await.unwrap(), b"right");
    }
}

/// Tenants share nothing.
#[derive(Debug, Clone, Copy)]
pub struct Tenants;

impl Tenants {
    /// The same bytes under two tenants are two blobs: each is listed, read and deleted alone.
    pub async fn check<B: Blob>(blob: &B) {
        let (a, b) = (Samples::tenant("site-a"), Samples::tenant("site-b"));
        let hash = Hash::of(b"shared bytes");
        blob.put(&a, hash, [b"shared bytes"]).await.unwrap();
        assert!(!blob.exists(&b, hash).await.unwrap());
        assert!(blob.get(&b, hash).await.unwrap().is_none());
        assert!(blob.list(&b).await.unwrap().is_empty());
        assert!(blob.put(&b, hash, [b"shared bytes"]).await.unwrap().created, "another tenant's copy is its own");
        assert_eq!(blob.delete(&a, hash).await.unwrap(), Deleted::Removed(Bytes(12)));
        assert!(blob.exists(&b, hash).await.unwrap());
        assert!(!blob.exists(&a, hash).await.unwrap());
    }
}

/// Listing and deleting.
#[derive(Debug, Clone, Copy)]
pub struct Listing;

impl Listing {
    /// A listing is every blob of the tenant, sorted by hash, and a delete is exact.
    pub async fn check<B: Blob>(blob: &B) {
        let t = Samples::tenant("site-a");
        assert!(blob.list(&t).await.unwrap().is_empty());
        let mut expected: Vec<Listed> = Vec::new();
        for text in ["one", "two", "three", "four"] {
            let hash = Hash::of(text);
            blob.put(&t, hash, [text.as_bytes()]).await.unwrap();
            expected.push(Listed { hash, size: Bytes(text.len() as u64) });
        }
        expected.sort();
        assert_eq!(blob.list(&t).await.unwrap(), expected);
        let gone = expected.remove(0);
        assert_eq!(blob.delete(&t, gone.hash).await.unwrap(), Deleted::Removed(gone.size));
        assert_eq!(blob.delete(&t, gone.hash).await.unwrap(), Deleted::Absent);
        assert_eq!(blob.list(&t).await.unwrap(), expected);
    }
}

/// Runs every case of the blob port's suite against the adapter `$make` builds: a closure that returns the
/// adapter, fresh and empty, with whatever must outlive it (a temporary folder, or `()`) as a
/// pair. Each case is a test of its own, so each has its own budget.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::suite!(@case $make, round_trips_a_blob, RoundTrip);
        $crate::suite!(@case $make, round_trips_the_empty_and_a_large_blob, Sizes);
        $crate::suite!(@case $make, refuses_bytes_that_do_not_hash_to_the_name, Verified);
        $crate::suite!(@case $make, keeps_tenants_apart, Tenants);
        $crate::suite!(@case $make, lists_and_deletes_exactly, Listing);
    };
    (@case $make:expr, $name:ident, $case:ident) => {
        #[test]
        fn $name() {
            let (blob, _guard) = ($make)();
            $crate::pollster::block_on($crate::$case::check(&blob));
        }
    };
}
