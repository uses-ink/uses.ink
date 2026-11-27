//! Unpadded base64url and Crockford base32, each with exactly one spelling for a value.

use std::fmt;

/// Text that is not the one spelling of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Undecodable {
    /// The text has the wrong length.
    Length {
        /// The length a value of this size has.
        expected: usize,
        /// The length found.
        found: usize,
    },
    /// A character outside the alphabet, at this offset.
    Character {
        /// The offset.
        at: usize,
    },
    /// The unused bits at the end of the last character are not zero, so the text would decode
    /// and then encode to something else.
    Tail,
}

impl fmt::Display for Undecodable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, found } => write!(out, "{found} characters where {expected} are needed"),
            Self::Character { at } => write!(out, "character {at} is outside the base64url alphabet"),
            Self::Tail => out.write_str("the unused bits of the last character are not zero"),
        }
    }
}

impl std::error::Error for Undecodable {}

/// Unpadded base64url (RFC 4648 section 5), strict: what decodes encodes back to the same text.
#[derive(Clone, Copy, Debug)]
pub struct Base64;

impl Base64 {
    const ALPHABET: &'static [u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    /// How many characters `bytes` bytes take.
    pub const fn length(bytes: usize) -> usize {
        (bytes * 8).div_ceil(6)
    }

    /// `bytes` as text.
    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(Self::length(bytes.len()));
        let (mut acc, mut bits) = (0_u32, 0_u32);
        for byte in bytes {
            acc = (acc << 8) | u32::from(*byte);
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                out.push(Self::symbol((acc >> bits) & 63));
            }
        }
        if bits > 0 {
            out.push(Self::symbol((acc << (6 - bits)) & 63));
        }
        out
    }

    fn symbol(index: u32) -> char {
        usize::try_from(index).ok().and_then(|at| Self::ALPHABET.get(at)).map_or('A', |byte| char::from(*byte))
    }

    fn value(byte: u8) -> Option<u32> {
        Self::ALPHABET.iter().position(|symbol| *symbol == byte).and_then(|at| u32::try_from(at).ok())
    }

    /// The `N` bytes `text` spells, when it is their one spelling.
    pub fn decode<const N: usize>(text: &str) -> Result<[u8; N], Undecodable> {
        let expected = Self::length(N);
        if text.len() != expected {
            return Err(Undecodable::Length { expected, found: text.len() });
        }
        let mut bytes = Vec::with_capacity(N);
        let (mut acc, mut bits) = (0_u32, 0_u32);
        for (at, byte) in text.bytes().enumerate() {
            acc = (acc << 6) | Self::value(byte).ok_or(Undecodable::Character { at })?;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                bytes.push(u8::try_from((acc >> bits) & 255).unwrap_or_default());
            }
        }
        if acc & ((1 << bits) - 1) != 0 {
            return Err(Undecodable::Tail);
        }
        <[u8; N]>::try_from(bytes).map_err(|_| Undecodable::Length { expected, found: text.len() })
    }
}

/// Crockford base32 (no `I`, `L`, `O`, `U`), for what a person reads aloud and compares.
#[derive(Clone, Copy, Debug)]
pub struct Crockford;

impl Crockford {
    /// The alphabet, in value order.
    pub const ALPHABET: &'static [u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    /// The first `chars` characters of `bytes` as text.
    pub fn encode(bytes: &[u8], chars: usize) -> String {
        let mut out = String::with_capacity(chars);
        let (mut acc, mut bits) = (0_u32, 0_u32);
        for byte in bytes {
            acc = (acc << 8) | u32::from(*byte);
            bits += 8;
            while bits >= 5 {
                if out.len() >= chars {
                    return out;
                }
                bits -= 5;
                let at = usize::try_from((acc >> bits) & 31).unwrap_or_default();
                out.push(Self::ALPHABET.get(at).map_or('0', |symbol| char::from(*symbol)));
            }
            acc &= (1 << bits) - 1;
        }
        out
    }

    /// The value of one hexadecimal digit, upper case only (`%7E` is the form, `%7e` is not).
    pub fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    /// The value of one character, upper case only.
    pub fn value(byte: u8) -> Option<u8> {
        Self::ALPHABET.iter().position(|symbol| *symbol == byte).and_then(|at| u8::try_from(at).ok())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn base64_round_trips_and_has_one_spelling() {
        assert_eq!(Base64::encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(Base64::decode::<2>("-_8"), Ok([0xfb, 0xff]));
        assert_eq!(Base64::decode::<2>("-_9"), Err(Undecodable::Tail));
        assert_eq!(Base64::decode::<2>("-_"), Err(Undecodable::Length { expected: 3, found: 2 }));
        assert_eq!(Base64::decode::<2>("-+8"), Err(Undecodable::Character { at: 1 }));
        assert_eq!(Base64::length(16), 22);
        assert_eq!(Base64::length(32), 43);
        assert_eq!(Base64::length(64), 86);
        let bytes: Vec<u8> = (0..16_u8).map(|n| n.wrapping_mul(37).wrapping_add(11)).collect();
        assert_eq!(Base64::decode::<16>(&Base64::encode(&bytes)), Ok(<[u8; 16]>::try_from(bytes).unwrap_or_default()));
    }

    #[test]
    fn crockford_takes_the_leading_bits() {
        assert_eq!(Crockford::encode(&[0xff, 0xff], 3), "ZZZ");
        assert_eq!(Crockford::encode(&[0, 0, 0], 4), "0000");
        assert_eq!(Crockford::value(b'Z'), Some(31));
        assert_eq!(Crockford::value(b'z'), None);
        assert_eq!(Crockford::value(b'I'), None);
    }
}
