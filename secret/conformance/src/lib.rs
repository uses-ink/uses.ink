//! The conformance suite of the secret port: each case is a type with a `check` over any
//! [`Secret`], and [`suite!`] turns every case into a test.
//!
//! ```ignore
//! secret_conformance::suite!(|| (MyAdapter::ephemeral(), ()));
//! ```

use secret::{Binding, Error, Plain, Secret, Sealed};

#[doc(hidden)]
pub use pollster;

/// What is sealed opens, for the row it was sealed to.
#[derive(Debug, Clone, Copy)]
pub struct RoundTrip;

impl RoundTrip {
    /// Any bytes, including none and non-text, come back unchanged, and the seal shows nothing.
    pub async fn check<S: Secret>(secret: &S) {
        let row = Binding::from("credential-1");
        for plain in [b"ghp_0123456789".to_vec(), Vec::new(), vec![0, 255, 0, 1], vec![7; 100_000]] {
            let sealed = secret.seal(&Plain::from(plain.clone()), &row).await.unwrap();
            assert!(plain.len() < 4 || !sealed.as_ref().windows(plain.len()).any(|window| window == plain), "the plaintext shows in the seal");
            assert_eq!(secret.open(&sealed, &row).await.unwrap().expose(), plain);
        }
    }
}

/// Every seal is fresh.
#[derive(Debug, Clone, Copy)]
pub struct Fresh;

impl Fresh {
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
    /// A seal made by one adapter instance does not open in another that holds a different key.
    pub async fn check<S: Secret>(one: &S, other: &S) {
        let row = Binding::from("credential-1");
        let sealed = one.seal(&Plain::from("token"), &row).await.unwrap();
        assert!(other.open(&sealed, &row).await.is_err(), "a second key opened the first's seal");
    }
}

/// Runs every case of the secret port's suite against the adapter `$make` builds: a closure that
/// returns the adapter and whatever must outlive it (a temporary folder, or `()`) as a pair, each
/// call with a key of its own. Each case is a test of its own, so each has its own budget.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::suite!(@case $make, opens_what_it_sealed, RoundTrip);
        $crate::suite!(@case $make, seals_afresh_every_time, Fresh);
        $crate::suite!(@case $make, refuses_another_row_and_any_change, Bound);
        $crate::suite!(@case $make, refuses_bytes_it_never_sealed, Garbage);
        #[test]
        fn keeps_each_key_to_itself() {
            let ((one, _first), (other, _second)) = (($make)(), ($make)());
            $crate::pollster::block_on($crate::Keys::check(&one, &other));
        }
    };
    (@case $make:expr, $name:ident, $case:ident) => {
        #[test]
        fn $name() {
            let (secret, _guard) = ($make)();
            $crate::pollster::block_on($crate::$case::check(&secret));
        }
    };
}
