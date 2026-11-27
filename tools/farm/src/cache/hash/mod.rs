//! A 128-bit content hash for the result cache: FNV-1a over 128 bits, fed in length-prefixed
//! fields so `("ab", "c")` and `("a", "bc")` differ. It guards against a stale pass, not against
//! an adversary, and it keeps the farm free of a hashing dependency (decision 0047).

pub(crate) mod key;

use std::fmt;
use std::str::FromStr;

/// A finished hash: 32 hexadecimal digits when shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Hash(u128);

/// A hash being fed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hasher(u128);

impl Default for Hasher {
    fn default() -> Self {
        Self(Self::OFFSET)
    }
}

impl Hasher {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    /// Adds raw bytes.
    pub(crate) fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        self.0 = bytes.iter().fold(self.0, |hash, byte| (hash ^ u128::from(*byte)).wrapping_mul(Self::PRIME));
        self
    }

    /// Adds a field: its length, then its bytes.
    pub(crate) fn field(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes(&(bytes.len() as u64).to_le_bytes()).bytes(bytes)
    }

    /// The hash of everything fed.
    pub(crate) const fn finish(&self) -> Hash {
        Hash(self.0)
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:032x}", self.0)
    }
}

impl FromStr for Hash {
    type Err = std::num::ParseIntError;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        u128::from_str_radix(given, 16).map(Self)
    }
}

impl serde::Serialize for Hash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Hash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let given = String::deserialize(deserializer)?;
        given.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_do_not_run_together_and_a_hash_round_trips() {
        let one = Hasher::default().field(b"ab").field(b"c").finish();
        let two = Hasher::default().field(b"a").field(b"bc").finish();
        assert_ne!(one, two);
        assert_eq!(one, Hasher::default().field(b"ab").field(b"c").finish());
        assert_eq!(one.to_string().len(), 32);
        assert_eq!(one.to_string().parse::<Hash>().unwrap(), one);
    }
}
