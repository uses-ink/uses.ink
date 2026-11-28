//! The conformance suite of the blob port: each case is a type with a `check` over any [`Blob`].
//! [`Case`] lists them, with a name each, as async functions any executor can drive (a Worker's
//! event loop included, which `pollster` cannot), and [`suite!`] turns every case into a native
//! test.
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

/// A blob that no adapter holds in one piece.
#[derive(Debug, Clone, Copy)]
pub struct Beyond;

impl Beyond {
    /// The size the cases of this type use: past every single-request limit an adapter has (R2's
    /// part is 8 MiB here), so it is stored in parts.
    pub const SIZE: usize = 20 << 20;

    /// The blob's bytes: a run that does not repeat at the size of any part.
    pub fn bytes(size: usize, seed: u8) -> Vec<u8> {
        (0..size).map(|n| u8::try_from(n % 251).unwrap().wrapping_add(seed)).collect()
    }

    /// Twenty mebibytes in chunks of an odd size round-trip, hash for hash.
    pub async fn check<B: Blob>(blob: &B) {
        let t = Samples::tenant("site-a");
        let big = Self::bytes(Self::SIZE, 1);
        let hash = Hash::of(&big);
        let stored = blob.put(&t, hash, big.chunks(1_000_003)).await.unwrap();
        assert_eq!((stored.size, stored.created), (Bytes(Self::SIZE as u64), true));
        assert_eq!(blob.stat(&t, hash).await.unwrap(), Some(Bytes(Self::SIZE as u64)));
        let mut read = blob.get(&t, hash).await.unwrap().unwrap();
        let mut joined = Vec::with_capacity(Self::SIZE);
        while let Some(chunk) = read.chunk().await.unwrap() {
            joined.extend_from_slice(&chunk);
        }
        assert!(Hash::of(&joined) == hash, "the bytes read are not the bytes written");
        let again = blob.put(&t, hash, big.chunks(3_000_017)).await.unwrap();
        assert_eq!((again.size, again.created), (Bytes(Self::SIZE as u64), false));
        assert_eq!(blob.list(&t).await.unwrap(), vec![Listed { hash, size: Bytes(Self::SIZE as u64) }]);
    }
}

/// A large write that fails leaves nothing.
#[derive(Debug, Clone, Copy)]
pub struct Aborted;

impl Aborted {
    /// Twelve mebibytes that hash to another name are refused, and neither the name nor the
    /// bytes' own hash is held or listed afterwards.
    pub async fn check<B: Blob>(blob: &B) {
        let t = Samples::tenant("site-a");
        let bytes = Beyond::bytes(12 << 20, 2);
        let named = Hash::of(Beyond::bytes(12 << 20, 3));
        let refused = blob.put(&t, named, bytes.chunks(999_983)).await.unwrap_err();
        assert!(
            matches!(refused, Error::Mismatch { expected, found } if expected == named && found == Hash::of(&bytes)),
            "{refused}"
        );
        assert!(!blob.exists(&t, named).await.unwrap());
        assert!(!blob.exists(&t, Hash::of(&bytes)).await.unwrap());
        assert!(blob.list(&t).await.unwrap().is_empty());
        assert!(blob.get(&t, named).await.unwrap().is_none());
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

/// Lists the cases once, for the macros that need them: each entry is the [`Case`] variant, the type
/// that holds the check, its method, and the name of the test.
#[doc(hidden)]
#[macro_export]
macro_rules! cases {
    ($callback:path, $($lead:tt)*) => {
        $callback! {
            $($lead)*
            [
                (RoundTrips, RoundTrip, check, round_trips_a_blob),
                (Grows, Sizes, check, round_trips_the_empty_and_a_large_blob),
                (Parted, Beyond, check, round_trips_a_blob_past_one_request),
                (Verifies, Verified, check, refuses_bytes_that_do_not_hash_to_the_name),
                (Aborts, Aborted, check, leaves_nothing_of_a_large_write_that_fails),
                (Apart, Tenants, check, keeps_tenants_apart),
                (Lists, Listing, check, lists_and_deletes_exactly)
            ]
        }
    };
}

macro_rules! define {
    ([$(($variant:ident, $case:ident, $method:ident, $name:ident)),*]) => {
        /// One case of the suite.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Case {
            $(
                #[doc = concat!("`", stringify!($case), "::", stringify!($method), "`.")]
                $variant,
            )*
        }

        impl Case {
            /// Every case, in the order the suite runs them.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            /// Its name: the name of its native test.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($name),)*
                }
            }

            /// The case named `name`, if there is one.
            pub fn named(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|case| case.name() == name)
            }

            /// Runs it against `blob`, which must be fresh and empty. It panics at the first surprise.
            pub async fn run<B: Blob>(self, blob: &B) {
                match self {
                    $(Self::$variant => $case::$method(blob).await,)*
                }
            }
        }
    };
}

cases!(define,);

/// The native tests of [`suite!`], one per case.
#[doc(hidden)]
#[macro_export]
macro_rules! tests {
    ($make:expr, [$(($variant:ident, $case:ident, $method:ident, $name:ident)),*]) => {
        $(
            #[test]
            fn $name() {
                let (blob, _guard) = ($make)();
                $crate::pollster::block_on($crate::Case::$variant.run(&blob));
            }
        )*
    };
}

/// Runs every case of the blob port's suite against the adapter `$make` builds: a closure that returns the
/// adapter, fresh and empty, with whatever must outlive it (a temporary folder, or `()`) as a
/// pair. Each case is a test of its own, so each has its own budget. The same cases are
/// [`Case::ALL`], for an executor that is not `pollster`.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::cases!($crate::tests, $make,);
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_case_has_its_own_name() {
        let mut names: Vec<&str> = Case::ALL.iter().map(|case| case.name()).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
        assert_eq!(count, 7);
        assert!(Case::ALL.iter().all(|case| Case::named(case.name()) == Some(*case)));
    }
}
