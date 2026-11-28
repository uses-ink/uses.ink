//! The conformance suite of the secret port: each case is a type with a `check` over any
//! [`Secret`]. [`Case`] lists them, with a name each, as async functions any executor can drive (a
//! Worker's event loop included, which `pollster` cannot), and [`suite!`] turns every case into a
//! native test.
//!
//! ```ignore
//! secret_conformance::suite!(|| (MyAdapter::ephemeral(), ()));
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use secret::{Binding, Error, Plain, Sealed, Secret};

#[doc(hidden)]
pub use pollster;

/// What is sealed opens, for the row it was sealed to.
#[derive(Debug, Clone, Copy)]
pub struct RoundTrip;

impl RoundTrip {
    /// The check, on the pair's first adapter.
    pub async fn check_pair<S: Secret>(pair: Pair<'_, S>) {
        Self::check(pair.one).await;
    }

    /// Any bytes, including none and non-text, come back unchanged, and the seal shows nothing.
    pub async fn check<S: Secret>(secret: &S) {
        let row = Binding::from("credential-1");
        for plain in [b"ghp_0123456789".to_vec(), Vec::new(), vec![0, 255, 0, 1], vec![7; 100_000]] {
            let sealed = secret.seal(&Plain::from(plain.clone()), &row).await.unwrap();
            assert!(
                plain.len() < 4 || !sealed.as_ref().windows(plain.len()).any(|window| window == plain),
                "the plaintext shows in the seal"
            );
            assert_eq!(secret.open(&sealed, &row).await.unwrap().expose(), plain);
        }
    }
}

/// Every seal is fresh.
#[derive(Debug, Clone, Copy)]
pub struct Fresh;

impl Fresh {
    /// The check, on the pair's first adapter.
    pub async fn check_pair<S: Secret>(pair: Pair<'_, S>) {
        Self::check(pair.one).await;
    }

    /// Sealing one credential twice gives two different values, both of which open.
    pub async fn check<S: Secret>(secret: &S) {
        let row = Binding::from("credential-1");
        let plain = Plain::from("same");
        let first = secret.seal(&plain, &row).await.unwrap();
        let second = secret.seal(&plain, &row).await.unwrap();
        assert_ne!(first, second, "a repeated nonce");
        assert_eq!(secret.open(&second, &row).await.unwrap().expose(), b"same");
    }
}

/// A sealed value belongs to its row.
#[derive(Debug, Clone, Copy)]
pub struct Bound;

impl Bound {
    /// The check, on the pair's first adapter.
    pub async fn check_pair<S: Secret>(pair: Pair<'_, S>) {
        Self::check(pair.one).await;
    }

    /// Opening for another row is refused, as is any change to the sealed bytes.
    pub async fn check<S: Secret>(secret: &S) {
        let row = Binding::from("credential-1");
        let sealed = secret.seal(&Plain::from("token"), &row).await.unwrap();
        let moved = secret.open(&sealed, &Binding::from("credential-2")).await.unwrap_err();
        assert!(matches!(moved, Error::Refused), "{moved}");
        for at in 0..sealed.as_ref().len() {
            let mut bytes = sealed.as_ref().to_vec();
            if let Some(byte) = bytes.get_mut(at) {
                *byte ^= 1;
            }
            let altered = secret.open(&Sealed::from(bytes), &row).await;
            assert!(altered.is_err(), "byte {at} of the seal can change and it still opens");
        }
    }
}

/// Bytes that were never sealed.
#[derive(Debug, Clone, Copy)]
pub struct Garbage;

impl Garbage {
    /// The check, on the pair's first adapter.
    pub async fn check_pair<S: Secret>(pair: Pair<'_, S>) {
        Self::check(pair.one).await;
    }

    /// Nothing, a short string and a long one are refused as typed errors, never a panic.
    pub async fn check<S: Secret>(secret: &S) {
        let row = Binding::from("credential-1");
        for bytes in [Vec::new(), vec![1], vec![0; 10], vec![0; 200]] {
            let refused = secret.open(&Sealed::from(bytes), &row).await.unwrap_err();
            assert!(matches!(refused, Error::Refused | Error::Malformed), "{refused}");
        }
    }
}

/// Two keys never open each other's seals.
#[derive(Debug, Clone, Copy)]
pub struct Keys;

impl Keys {
    /// The check, on the pair.
    pub async fn check_pair<S: Secret>(pair: Pair<'_, S>) {
        Self::check(pair.one, pair.other).await;
    }

    /// A seal made by one adapter instance does not open in another that holds a different key.
    pub async fn check<S: Secret>(one: &S, other: &S) {
        let row = Binding::from("credential-1");
        let sealed = one.seal(&Plain::from("token"), &row).await.unwrap();
        assert!(other.open(&sealed, &row).await.is_err(), "a second key opened the first's seal");
    }
}

/// Lists the cases once, for the macros that need them: each entry is the [`Case`] variant, the type
/// that holds the check, and the name of the test.
#[doc(hidden)]
#[macro_export]
macro_rules! cases {
    ($callback:path, $($lead:tt)*) => {
        $callback! {
            $($lead)*
            [
                (Opens, RoundTrip, opens_what_it_sealed),
                (Fresh, Fresh, seals_afresh_every_time),
                (Bound, Bound, refuses_another_row_and_any_change),
                (Garbage, Garbage, refuses_bytes_it_never_sealed),
                (Keys, Keys, keeps_each_key_to_itself)
            ]
        }
    };
}

/// What a case is run on: one adapter, and another that holds a different key.
#[derive(Debug, Clone, Copy)]
pub struct Pair<'a, S> {
    /// The adapter every case but one uses.
    pub one: &'a S,
    /// An adapter with another key, for the case that tells keys apart.
    pub other: &'a S,
}

macro_rules! define {
    ([$(($variant:ident, $case:ident, $name:ident)),*]) => {
        /// One case of the suite.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum Case {
            $(
                #[doc = concat!("`", stringify!($case), "`.")]
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

            /// Runs it on `pair`. It panics at the first surprise.
            pub async fn run<S: Secret>(self, pair: Pair<'_, S>) {
                match self {
                    $(Self::$variant => $case::check_pair(pair).await,)*
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
    ($make:expr, [$(($variant:ident, $case:ident, $name:ident)),*]) => {
        $(
            #[test]
            fn $name() {
                let ((one, _first), (other, _second)) = (($make)(), ($make)());
                $crate::pollster::block_on($crate::Case::$variant.run($crate::Pair { one: &one, other: &other }));
            }
        )*
    };
}

/// Runs every case of the secret port's suite against the adapter `$make` builds: a closure that
/// returns the adapter and whatever must outlive it (a temporary folder, or `()`) as a pair, each
/// call with a key of its own. Each case is a test of its own, so each has its own budget. The same
/// cases are [`Case::ALL`], for an executor that is not `pollster`.
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
        assert_eq!(count, 5);
        assert!(Case::ALL.iter().all(|case| Case::named(case.name()) == Some(*case)));
    }
}
