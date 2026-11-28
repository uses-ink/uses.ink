//! The `Authorization: Ink-Sig-1 ...` header: one wire form per message.

use std::fmt;
use std::str::FromStr;

use crate::alphabet::{Base64, Undecodable};
use crate::key::KeyId;

/// The five fields of the header, in wire order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Field {
    /// `k`: the key id.
    Key,
    /// `c`: when it was made.
    Created,
    /// `n`: the nonce.
    Nonce,
    /// `d`: the BLAKE3 of the body.
    Digest,
    /// `s`: the signature.
    Signature,
}

impl Field {
    /// The fields in the order they are written.
    pub const ORDER: [Self; 5] = [Self::Key, Self::Created, Self::Nonce, Self::Digest, Self::Signature];

    /// The one-letter name on the wire.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Key => "k=",
            Self::Created => "c=",
            Self::Nonce => "n=",
            Self::Digest => "d=",
            Self::Signature => "s=",
        }
    }
}

/// A `c` that is not a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CreatedError {
    /// No digits.
    Empty,
    /// A character that is not a decimal digit, at this offset.
    Digit {
        /// The offset in the field.
        at: usize,
    },
    /// A leading zero (and so `0` itself).
    Zero,
    /// More digits than [`Created::DIGITS`].
    Long {
        /// How many.
        found: usize,
    },
}

impl fmt::Display for CreatedError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("the time has no digits"),
            Self::Digit { at } => write!(out, "character {at} of the time is not a decimal digit"),
            Self::Zero => out.write_str("the time starts with a zero"),
            Self::Long { found } => write!(out, "the time has {found} digits, over {}", Created::DIGITS),
        }
    }
}

impl std::error::Error for CreatedError {}

/// `created`: unix seconds, decimal, no sign, no leading zero, at most [`Created::DIGITS`] digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Created(u64);

/// How far a request's time is from the server's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skew {
    /// How many seconds apart.
    pub distance: u64,
    /// How many are allowed.
    pub window: u64,
}

impl Created {
    /// The most digits a time has.
    pub const DIGITS: usize = 10;

    /// A time from unix seconds.
    pub const fn new(seconds: u64) -> Self {
        Self(seconds)
    }

    /// The unix seconds.
    pub const fn seconds(self) -> u64 {
        self.0
    }

    /// Whether this time is within `window` seconds of `now` (unix seconds), either side.
    pub const fn within(self, now: u64, window: u64) -> Result<(), Skew> {
        let distance = self.0.abs_diff(now);
        if distance <= window { Ok(()) } else { Err(Skew { distance, window }) }
    }
}

impl fmt::Display for Created {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

impl FromStr for Created {
    type Err = CreatedError;

    fn from_str(text: &str) -> Result<Self, CreatedError> {
        if text.is_empty() {
            return Err(CreatedError::Empty);
        }
        if let Some(at) = text.bytes().position(|byte| !byte.is_ascii_digit()) {
            return Err(CreatedError::Digit { at });
        }
        if text.starts_with('0') {
            return Err(CreatedError::Zero);
        }
        if text.len() > Self::DIGITS {
            return Err(CreatedError::Long { found: text.len() });
        }
        text.parse().map(Self).map_err(|_| CreatedError::Empty)
    }
}

/// A fresh value of 16 bytes, one draw, so a request is never the same twice.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nonce([u8; 16]);

impl Nonce {
    /// A nonce from one draw.
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for Nonce {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Nonce({self})")
    }
}

impl fmt::Display for Nonce {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&Base64::encode(&self.0))
    }
}

/// The BLAKE3 of a request body, of the empty string when there is none. Two digests compare in
/// constant time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Digest(blake3::Hash);

/// A body that is not the one that was signed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tampered;

impl fmt::Display for Tampered {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the body is not the one that was signed")
    }
}

impl std::error::Error for Tampered {}

impl Digest {
    /// The digest of `body`.
    pub fn of(body: &[u8]) -> Self {
        Self(blake3::hash(body))
    }

    /// The 32 bytes.
    pub const fn bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Digest({self})")
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&Base64::encode(self.0.as_bytes()))
    }
}

/// An Ed25519 signature: 64 bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature([u8; 64]);

impl Signature {
    /// A signature from its 64 bytes.
    pub const fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub const fn bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Signature({self})")
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&Base64::encode(&self.0))
    }
}

/// Everything but the signature that the signature covers (besides the request): the key, the
/// time, the nonce and the body's digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The key id.
    pub key: KeyId,
    /// When it was made.
    pub created: Created,
    /// The nonce.
    pub nonce: Nonce,
    /// The digest of the body.
    pub digest: Digest,
}

/// A header that is not the `Ink-Sig-1` profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Malformed {
    /// Not ASCII text.
    Text,
    /// Does not start with `Ink-Sig-1 `.
    Scheme,
    /// A field's name is not where it should be.
    Label {
        /// The field.
        field: Field,
        /// The offset.
        at: usize,
    },
    /// A field's value is not the one spelling of its bytes.
    Encoding {
        /// The field.
        field: Field,
        /// The offset of its value.
        at: usize,
        /// Why not.
        source: Undecodable,
    },
    /// The time is not a time.
    Time {
        /// The offset of its value.
        at: usize,
        /// Why not.
        source: CreatedError,
    },
    /// A field is not followed by a comma and one space.
    Separator {
        /// The offset.
        at: usize,
    },
    /// Something after the signature.
    Trailing {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for Malformed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => out.write_str("the header is not ASCII text"),
            Self::Scheme => write!(out, "the header does not start with `{}`", Signed::SCHEME),
            Self::Label { field, at } => write!(out, "`{}` was expected at byte {at}", field.label()),
            Self::Encoding { field, at, .. } => {
                write!(out, "the value of `{}` at byte {at} is not valid", field.label())
            }
            Self::Time { at, .. } => write!(out, "the time at byte {at} is not valid"),
            Self::Separator { at } => write!(out, "a comma and one space were expected at byte {at}"),
            Self::Trailing { at } => write!(out, "the header goes on after the signature, at byte {at}"),
        }
    }
}

impl std::error::Error for Malformed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encoding { source, .. } => Some(source),
            Self::Time { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The signed-request header: `Ink-Sig-1 k=<kid>, c=<created>, n=<nonce>, d=<digest>, s=<signature>`.
///
/// The parser accepts exactly this form: the scheme, the five names in this order, one space after
/// each comma, unpadded base64url that decodes and encodes back to the same text, nothing before
/// or after. So there is one wire form per message, and what parses prints back to the bytes it
/// came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signed {
    /// What the signature covers besides the request.
    pub claim: Claim,
    /// The signature over the signed text.
    pub signature: Signature,
}

/// A position in the header text.
struct Cursor<'a> {
    text: &'a str,
    at: usize,
}

impl Cursor<'_> {
    fn literal(&mut self, expected: &str) -> bool {
        let found = self.text.get(self.at..).is_some_and(|rest| rest.starts_with(expected));
        if found {
            self.at += expected.len();
        }
        found
    }

    fn label(&mut self, field: Field) -> Result<(), Malformed> {
        if self.literal(field.label()) { Ok(()) } else { Err(Malformed::Label { field, at: self.at }) }
    }

    fn separator(&mut self) -> Result<(), Malformed> {
        if self.literal(", ") { Ok(()) } else { Err(Malformed::Separator { at: self.at }) }
    }

    fn bytes<const N: usize>(&mut self, field: Field) -> Result<[u8; N], Malformed> {
        self.label(field)?;
        let at = self.at;
        let end = at + Base64::length(N);
        let piece = self.text.get(at..end.min(self.text.len())).unwrap_or_default();
        let decoded = Base64::decode::<N>(piece).map_err(|source| Malformed::Encoding { field, at, source })?;
        self.at = end;
        Ok(decoded)
    }

    fn created(&mut self) -> Result<Created, Malformed> {
        self.label(Field::Created)?;
        let at = self.at;
        let rest = self.text.get(at..).unwrap_or_default();
        let length = rest.bytes().take_while(u8::is_ascii_digit).count();
        let created =
            rest.get(..length).unwrap_or_default().parse().map_err(|source| Malformed::Time { at, source })?;
        self.at += length;
        Ok(created)
    }
}

impl Signed {
    /// The scheme and the space after it.
    pub const SCHEME: &'static str = "Ink-Sig-1 ";

    /// Reads `header`, the bytes of an `Authorization` value.
    pub fn parse(header: &[u8]) -> Result<Self, Malformed> {
        let text = std::str::from_utf8(header).ok().filter(|text| text.is_ascii()).ok_or(Malformed::Text)?;
        let mut cursor = Cursor { text, at: 0 };
        if !cursor.literal(Self::SCHEME) {
            return Err(Malformed::Scheme);
        }
        let key = KeyId::new(cursor.bytes(Field::Key)?);
        cursor.separator()?;
        let created = cursor.created()?;
        cursor.separator()?;
        let nonce = Nonce(cursor.bytes(Field::Nonce)?);
        cursor.separator()?;
        let digest = Digest(blake3::Hash::from_bytes(cursor.bytes(Field::Digest)?));
        cursor.separator()?;
        let signature = Signature(cursor.bytes(Field::Signature)?);
        if cursor.at != text.len() {
            return Err(Malformed::Trailing { at: cursor.at });
        }
        Ok(Self { claim: Claim { key, created, nonce, digest }, signature })
    }

    /// Whether `body` is the one the digest says. A mismatch means the body changed after it was
    /// signed. The comparison takes the same time wherever the digests differ.
    pub fn covers(&self, body: &[u8]) -> Result<(), Tampered> {
        if Digest::of(body) == self.claim.digest { Ok(()) } else { Err(Tampered) }
    }
}

impl fmt::Display for Signed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let claim = &self.claim;
        write!(
            out,
            "{}k={}, c={}, n={}, d={}, s={}",
            Self::SCHEME,
            claim.key,
            claim.created,
            claim.nonce,
            claim.digest,
            self.signature
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn valid() -> String {
        let claim = Claim {
            key: KeyId::new([1; 16]),
            created: Created::new(1_795_000_000),
            nonce: Nonce::new([2; 16]),
            digest: Digest::of(b"body"),
        };
        Signed { claim, signature: Signature::new([3; 64]) }.to_string()
    }

    #[test]
    fn a_header_prints_what_it_parsed_from() {
        let text = valid();
        assert_eq!(Signed::parse(text.as_bytes()).map(|signed| signed.to_string()), Ok(text.clone()));
        assert!(text.starts_with("Ink-Sig-1 k="));
        assert_eq!(text.matches(", ").count(), 4);
    }

    #[test]
    fn the_body_digest_is_checked_after_the_head() {
        let text = valid();
        let signed = Signed::parse(text.as_bytes()).unwrap();
        assert_eq!(signed.covers(b"body"), Ok(()));
        assert_eq!(signed.covers(b"bodz"), Err(Tampered));
    }

    #[test]
    fn each_part_of_the_form_is_checked() {
        let text = valid();
        let parse = |header: &str| Signed::parse(header.as_bytes());
        assert_eq!(parse(&text.replacen("Ink-Sig-1", "Ink-Sig-2", 1)), Err(Malformed::Scheme));
        assert_eq!(parse(&text.to_lowercase()), Err(Malformed::Scheme));
        assert!(matches!(parse(&text.replacen("k=", "K=", 1)), Err(Malformed::Label { field: Field::Key, .. })));
        assert!(matches!(parse(&text.replacen(", c=", ",c=", 1)), Err(Malformed::Separator { .. })));
        assert!(matches!(parse(&text.replacen(", c=", ",  c=", 1)), Err(Malformed::Label { .. })));
        assert!(matches!(parse(&format!("{text} ")), Err(Malformed::Trailing { .. })));
        assert!(matches!(parse(&format!("{text}A")), Err(Malformed::Trailing { .. } | Malformed::Encoding { .. })));
        assert!(matches!(
            parse(&text.replacen("c=1795", "c=01795", 1)),
            Err(Malformed::Time { source: CreatedError::Zero, .. })
        ));
        assert!(matches!(parse(&text.replacen("c=1795000000", "c=17950000000", 1)), Err(Malformed::Time { .. })));
        assert!(matches!(parse(&text.replacen("c=1795000000", "c=+795000000", 1)), Err(Malformed::Time { .. })));
        assert_eq!(Signed::parse(&[0xff]), Err(Malformed::Text));
        assert_eq!(parse("Ink-Sig-1 k=\u{e9}"), Err(Malformed::Text));
    }

    #[test]
    fn mutating_any_byte_never_accepts_a_second_spelling() {
        let text = valid();
        let bytes = text.as_bytes();
        let accepted = |candidate: &[u8]| {
            if let Ok(signed) = Signed::parse(candidate) {
                assert_eq!(signed.to_string().as_bytes(), candidate, "{}", String::from_utf8_lossy(candidate));
                assert!(!candidate.contains(&b'\n'));
            }
        };
        for at in 0..bytes.len() {
            for value in 0..=255_u8 {
                let mut flipped = bytes.to_vec();
                if let Some(slot) = flipped.get_mut(at) {
                    *slot = value;
                }
                accepted(&flipped);
                let mut inserted = bytes.to_vec();
                inserted.insert(at, value);
                accepted(&inserted);
            }
            let mut removed = bytes.to_vec();
            removed.remove(at);
            accepted(&removed);
        }
    }

    #[test]
    fn created_is_a_window_around_now() {
        let created = Created::new(1_000);
        assert_eq!(created.within(1_060, 60), Ok(()));
        assert_eq!(created.within(940, 60), Ok(()));
        assert_eq!(created.within(1_061, 60), Err(Skew { distance: 61, window: 60 }));
        assert_eq!(created.within(939, 60), Err(Skew { distance: 61, window: 60 }));
    }
}
