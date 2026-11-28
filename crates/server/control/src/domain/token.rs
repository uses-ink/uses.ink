//! The token a TXT record carries, and what a lookup of the record found.

use std::fmt;
use std::str::FromStr;

use control_rows::Digest;

/// A random 128-bit token as 32 lower-case hex digits: what the TXT record at
/// `_uses-ink.<name>` says to prove the name is a site's. It is published in DNS by the person who
/// proves, so it is not a secret; it is unguessable, so a stranger cannot have a name proved by a
/// token another claim holds.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token([u8; 16]);

impl Token {
    /// How many hex digits a token has.
    pub const DIGITS: usize = 32;

    /// A token from 16 bytes the caller drew.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Whether `other` is this token, compared in constant time.
    pub fn is(&self, other: &Self) -> bool {
        Self::digest(self) == Self::digest(other)
    }

    fn digest(&self) -> Digest {
        Digest::derive("uses.ink 2026-11-28 domain token", &self.0)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Token({self})")
    }
}

/// Text that is no token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenError {
    /// Not 32 characters long.
    Length {
        /// How long it is.
        found: usize,
    },
    /// A character that is not a lower-case hex digit.
    Digit,
}

impl fmt::Display for TokenError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a domain token is {} characters, not {found}", Token::DIGITS),
            Self::Digit => out.write_str("a domain token is lower-case hex digits"),
        }
    }
}

impl std::error::Error for TokenError {}

impl FromStr for Token {
    type Err = TokenError;

    fn from_str(text: &str) -> Result<Self, TokenError> {
        if text.len() != Self::DIGITS {
            return Err(TokenError::Length { found: text.len() });
        }
        let mut bytes = [0_u8; 16];
        for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
            let digit = |c: u8| match c {
                b'0'..=b'9' => Ok(c - b'0'),
                b'a'..=b'f' => Ok(c - b'a' + 10),
                _ => Err(TokenError::Digit),
            };
            let [high, low] = pair else { return Err(TokenError::Digit) };
            *byte = digit(*high)? << 4 | digit(*low)?;
        }
        Ok(Self(bytes))
    }
}

/// What a lookup of the TXT record found: how many records it read and the tokens among them.
/// The authority reads the records; the control plane decides whether they prove the claim.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Found {
    /// How many TXT records the name has.
    pub records: u64,
    /// The tokens among them, in the form the authority's record format carries.
    pub tokens: Vec<Token>,
}

impl Found {
    /// A lookup that read `records` records, `tokens` of which carried a token.
    pub const fn new(records: u64, tokens: Vec<Token>) -> Self {
        Self { records, tokens }
    }

    /// Whether any token found is `token`.
    pub fn has(&self, token: &Token) -> bool {
        self.tokens.iter().fold(false, |found, seen| found | seen.is(token))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_token_is_32_hex_digits_that_round_trip() {
        let token = Token::from_bytes([0xab; 16]);
        assert_eq!(token.to_string(), "ab".repeat(16));
        assert_eq!(token.to_string().parse::<Token>().unwrap(), token);
        assert_eq!("ab".parse::<Token>().unwrap_err(), TokenError::Length { found: 2 });
        assert_eq!("g".repeat(32).parse::<Token>().unwrap_err(), TokenError::Digit);
        assert_eq!("A".repeat(32).parse::<Token>().unwrap_err(), TokenError::Digit);
    }

    #[test]
    fn a_lookup_has_a_token_only_when_one_of_its_tokens_is_it() {
        let (one, two) = (Token::from_bytes([1; 16]), Token::from_bytes([2; 16]));
        let found = Found::new(3, vec![one]);
        assert!(found.has(&one) && !found.has(&two));
        assert!(!Found::default().has(&one));
    }
}
