//! The key.

use std::fmt;

use zeroize::Zeroizing;

use crate::error::{Entropy, Unhexed};

/// The 32-byte key, zeroed when dropped. It has no `Clone`, and its `Debug` shows nothing.
pub struct Key(Zeroizing<[u8; Self::LEN]>);

impl Key {
    /// The key's length in bytes.
    pub const LEN: usize = 32;

    /// A key of these bytes.
    pub const fn new(bytes: Zeroizing<[u8; Self::LEN]>) -> Self {
        Self(bytes)
    }

    /// A key from the operating system's randomness.
    pub fn generate() -> Result<Self, Entropy> {
        use rand_core::{OsRng, RngCore as _};
        let mut bytes = Zeroizing::new([0_u8; Self::LEN]);
        OsRng.try_fill_bytes(bytes.as_mut()).map_err(|source| Entropy { source })?;
        Ok(Self(bytes))
    }

    /// A key written as 64 hexadecimal digits, either case, with nothing around them: a trailing
    /// newline from a pipe is a refusal, not trimmed.
    pub fn from_hex(text: &str) -> Result<Self, Unhexed> {
        if text.len() != Self::LEN * 2 {
            return Err(Unhexed::Length { found: text.len() });
        }
        let mut bytes = Zeroizing::new([0_u8; Self::LEN]);
        for (at, pair) in text.as_bytes().chunks(2).enumerate() {
            let high = Self::digit(pair.first().copied(), at * 2)?;
            let low = Self::digit(pair.get(1).copied(), at * 2 + 1)?;
            if let Some(byte) = bytes.get_mut(at) {
                *byte = high << 4 | low;
            }
        }
        Ok(Self(bytes))
    }

    /// The value of one hexadecimal digit, which stood at offset `at`.
    fn digit(found: Option<u8>, at: usize) -> Result<u8, Unhexed> {
        match found {
            Some(byte @ b'0'..=b'9') => Ok(byte - b'0'),
            Some(byte @ b'a'..=b'f') => Ok(byte - b'a' + 10),
            Some(byte @ b'A'..=b'F') => Ok(byte - b'A' + 10),
            Some(_) | None => Err(Unhexed::Digit { at }),
        }
    }

    /// The bytes, for the adapter that keeps the key (writing a key file); sealing never needs them.
    pub fn expose(&self) -> &[u8; Self::LEN] {
        &self.0
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Key(<redacted>)")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn hex_reads_either_case_and_refuses_the_rest() {
        let key = Key::from_hex(&"0aF9".repeat(16)).unwrap();
        assert_eq!(&key.expose()[..4], [0x0a, 0xf9, 0x0a, 0xf9]);
        assert_eq!(Key::from_hex("00").unwrap_err(), Unhexed::Length { found: 2 });
        assert_eq!(Key::from_hex(&format!("{}\n", "0".repeat(63))).unwrap_err(), Unhexed::Digit { at: 63 });
        assert_eq!(Key::from_hex(&"0".repeat(65)).unwrap_err(), Unhexed::Length { found: 65 });
        assert_eq!(Key::from_hex(&format!("{}g", "0".repeat(63))).unwrap_err(), Unhexed::Digit { at: 63 });
        assert_eq!(format!("{:?}", Key::from_hex(&"1".repeat(64)).unwrap()), "Key(<redacted>)");
    }
}
