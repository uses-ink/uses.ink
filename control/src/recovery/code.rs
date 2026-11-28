//! A recovery code: 128 random bits a person keeps offline, written as 26 Crockford base32
//! characters and a check character, in groups of four.

use std::fmt;
use std::str::FromStr;

use secret::Plain;
use sig::Crockford;

use crate::signin::Digest;

/// A recovery code. It prints as `Code(<redacted>)`, is zeroed when dropped and cannot be cloned
/// or serialized; the only way to its text is [`Code::reveal`], which is called once, to show it.
/// Only [`Code::digest`] is stored.
pub struct Code(Plain);

/// Text that is no recovery code. It says how the text differs and never what it held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CodeError {
    /// Not 27 characters once dashes and spaces are taken out.
    Length {
        /// How many characters there are.
        found: usize,
    },
    /// A character outside the alphabet, at this position among the characters kept.
    Character {
        /// The position.
        at: usize,
    },
    /// The unused bits of the last character are not zero: not a code this server made.
    Tail,
    /// The check character does not match: a typo, found before the code is tried.
    Check,
}

impl fmt::Display for CodeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a recovery code has {} characters, not {found}", Code::LENGTH),
            Self::Character { at } => write!(out, "character {at} of the recovery code is not a letter or digit of the alphabet"),
            Self::Tail => out.write_str("the end of the recovery code is not one this server makes"),
            Self::Check => out.write_str("the check character of the recovery code does not match: look for a typo"),
        }
    }
}

impl std::error::Error for CodeError {}

impl Code {
    /// How many characters a code has, the check character included.
    pub const LENGTH: usize = 27;
    /// The data characters: 128 bits need 26 of five bits.
    const DATA: usize = 26;
    /// Crockford's check symbols: the 32 of the alphabet and five more, for a value modulo 37.
    const CHECKS: &'static [u8; 37] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ*~$=U";

    /// A code from 16 bytes the caller drew.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Plain::from(bytes.to_vec()))
    }

    fn bits(&self) -> [u8; 16] {
        let mut out = [0_u8; 16];
        for (slot, byte) in out.iter_mut().zip(self.0.expose()) {
            *slot = *byte;
        }
        out
    }

    fn check(bytes: [u8; 16]) -> char {
        let at = usize::try_from(u128::from_be_bytes(bytes) % 37).unwrap_or_default();
        Self::CHECKS.get(at).map_or('0', |symbol| char::from(*symbol))
    }

    /// The code as shown to its owner: `ABCD-EFGH-IJKL-MNPQ-RSTV-WXYZ-012`. Reading it is the one
    /// moment the code exists outside the digest, so every call is a place to look.
    pub fn reveal(&self) -> String {
        let bytes = self.bits();
        let mut padded = [0_u8; 17];
        padded[..16].copy_from_slice(&bytes);
        let mut text = Crockford::encode(&padded, Self::DATA);
        text.push(Self::check(bytes));
        let mut grouped = String::with_capacity(Self::LENGTH + 6);
        for (at, symbol) in text.chars().enumerate() {
            if at > 0 && at % 4 == 0 {
                grouped.push('-');
            }
            grouped.push(symbol);
        }
        grouped
    }

    /// What is stored: the BLAKE3 digest of the 128 bits under the recovery context. 128 random
    /// bits need no salt and no slow hash.
    pub fn digest(&self) -> Digest {
        Digest::derive("uses.ink 2026-11-27 recovery code v1", &self.bits())
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Code(<redacted>)")
    }
}

impl FromStr for Code {
    type Err = CodeError;

    /// Reads what a person typed: dashes, spaces and case are ignored, and `O`, `I` and `L` are
    /// read as `0`, `1` and `1`, as Crockford's alphabet says.
    fn from_str(text: &str) -> Result<Self, CodeError> {
        let kept: Vec<u8> = text
            .bytes()
            .filter(|byte| !matches!(byte, b'-' | b' '))
            .map(|byte| match byte.to_ascii_uppercase() {
                b'O' => b'0',
                b'I' | b'L' => b'1',
                other => other,
            })
            .collect();
        if kept.len() != Self::LENGTH {
            return Err(CodeError::Length { found: kept.len() });
        }
        let (mut acc, mut bits, mut bytes) = (0_u32, 0_u32, Vec::with_capacity(17));
        for (at, byte) in kept.iter().take(Self::DATA).enumerate() {
            acc = (acc << 5) | u32::from(Crockford::value(*byte).ok_or(CodeError::Character { at })?);
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                bytes.push(u8::try_from((acc >> bits) & 255).unwrap_or_default());
            }
            acc &= (1 << bits) - 1;
        }
        // 26 characters are 130 bits: the last two are padding and must be zero
        if acc != 0 {
            return Err(CodeError::Tail);
        }
        let bytes: [u8; 16] = bytes.try_into().map_err(|_| CodeError::Length { found: kept.len() })?;
        let check = kept.get(Self::DATA).copied().ok_or(CodeError::Length { found: kept.len() })?;
        if char::from(check) != Self::check(bytes) {
            return Err(if Self::CHECKS.contains(&check) { CodeError::Check } else { CodeError::Character { at: Self::DATA } });
        }
        Ok(Self::from_bytes(bytes))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_code_is_27_characters_in_groups_of_four_and_reads_back() {
        let code = Code::from_bytes([0xab; 16]);
        let shown = code.reveal();
        assert_eq!(shown.len(), 27 + 6);
        assert!(shown.split('-').take(6).all(|group| group.len() == 4));
        let read: Code = shown.parse().unwrap();
        assert_eq!(read.digest(), code.digest());
        assert_eq!(format!("{code:?}"), "Code(<redacted>)");
        // case, dashes, spaces and the look-alike letters are forgiven
        let loose = shown.to_ascii_lowercase().replace('-', " ").replace('0', "o").replace('1', "l");
        assert_eq!(loose.parse::<Code>().unwrap().digest(), code.digest());
    }

    #[test]
    fn every_value_has_one_spelling_and_a_typo_is_caught_by_the_check() {
        for seed in [0_u8, 1, 0x7f, 0xff] {
            let code = Code::from_bytes([seed; 16]);
            assert_eq!(code.reveal().parse::<Code>().unwrap().reveal(), code.reveal());
        }
        let shown = Code::from_bytes([0x5a; 16]).reveal();
        let mut chars: Vec<char> = shown.chars().collect();
        chars[3] = if chars[3] == '2' { '3' } else { '2' };
        let typo: String = chars.into_iter().collect();
        assert_eq!(typo.parse::<Code>().unwrap_err(), CodeError::Check);
        assert_eq!("ABCD".parse::<Code>().unwrap_err(), CodeError::Length { found: 4 });
        assert!(matches!("U".repeat(27).parse::<Code>().unwrap_err(), CodeError::Character { at: 0 }));
        // the last data character carries two padding bits: a value with them set is no code of ours
        let tail = format!("{}Z{}", "0".repeat(25), "0");
        assert_eq!(tail.parse::<Code>().unwrap_err(), CodeError::Tail);
    }

    #[test]
    fn digests_differ_by_code_and_by_context() {
        let (one, two) = (Code::from_bytes([1; 16]), Code::from_bytes([2; 16]));
        assert_ne!(one.digest(), two.digest());
        assert_ne!(one.digest(), Digest::derive("uses.ink 2026-11-27 session", &[1; 16]));
    }
}
