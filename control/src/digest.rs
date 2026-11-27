//! The digest a secret is stored as.

use std::fmt;

/// The BLAKE3 digest of a proof under a context string. Proofs are 256 random bits, so a digest of
/// one cannot be reversed or guessed at, and no salt or stretching is needed. Two digests are
/// compared in constant time.
#[derive(Clone, Copy)]
pub struct Digest(blake3::Hash);

impl Digest {
    /// The digest of `material` under `key`, a string that names what the digest is for.
    pub fn derive(key: &str, material: &[u8]) -> Self {
        Self(blake3::Hash::from(blake3::derive_key(key, material)))
    }

    /// The 32 bytes that are stored.
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl PartialEq for Digest {
    /// `blake3::Hash` compares in constant time.
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Digest {}

impl fmt::Debug for Digest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Digest(..)")
    }
}

/// Bytes that are no digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotDigest;

impl fmt::Display for NotDigest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a digest is 32 bytes")
    }
}

impl std::error::Error for NotDigest {}

impl TryFrom<&[u8]> for Digest {
    type Error = NotDigest;

    fn try_from(bytes: &[u8]) -> Result<Self, NotDigest> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| NotDigest)?;
        Ok(Self(blake3::Hash::from(bytes)))
    }
}
