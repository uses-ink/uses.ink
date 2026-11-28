//! The case of a rotation, beside the port's conformance suite.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use secret::{Binding, Error, Plain, Sealed, Secret};

use crate::Worker;

/// A rotation keeps what the old key sealed readable, and nothing else.
#[derive(Debug, Clone, Copy)]
pub struct Rotated;

impl Rotated {
    /// The key before the rotation.
    const OLD: &'static str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    /// The key after it.
    const NEW: &'static str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

    /// A third key, which neither adapter holds.
    const OTHER: &'static str = "0f0e0d0c0b0a09080706050403020100f0e0d0c0b0a09080706050403020100f";

    /// With the old key as the previous one, old seals open and new seals are made under the new
    /// key; without it, old seals are refused; a key that is neither is refused either way.
    pub async fn check() {
        let row = Binding::from("credential-1");
        let old = Worker::from_hex(Self::OLD, None).unwrap();
        let before: Sealed = old.seal(&Plain::from("token"), &row).await.unwrap();
        let rotating = Worker::from_hex(Self::NEW, Some("SEALING_KEY"))
            .unwrap()
            .previous(Self::OLD, Some("SEALING_KEY_PREVIOUS"))
            .unwrap();
        assert_eq!(rotating.open(&before, &row).await.unwrap().expose(), b"token");
        let after = rotating.seal(&Plain::from("token"), &row).await.unwrap();
        assert!(old.open(&after, &row).await.is_err(), "a new seal opened under the old key");
        let rotated = Worker::from_hex(Self::NEW, None).unwrap();
        assert_eq!(rotated.open(&after, &row).await.unwrap().expose(), b"token");
        let dropped = rotated.open(&before, &row).await.unwrap_err();
        assert!(matches!(dropped, Error::Refused), "{dropped}");
        let neither = Worker::from_hex(Self::OTHER, None).unwrap().previous(Self::OLD, None).unwrap();
        let strange = neither.open(&after, &row).await.unwrap_err();
        assert!(matches!(strange, Error::Refused), "{strange}");
    }
}
