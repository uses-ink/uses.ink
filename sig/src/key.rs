//! Keys: the type, the secret seed, the public half, the id derived from it and the fingerprint
//! a person compares.

use std::fmt;
use std::str::FromStr;

use zeroize::Zeroize;

use crate::base::{Base64, Crockford, Undecodable};

/// What a key is. Fixed when the key is enrolled and stored beside it; a request cannot name
/// another (`docs/auth.md` section 5, algorithm agility).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyType {
    /// Ed25519 (RFC 8032, the pure scheme), signing the `Ink-Sig-1` text.
    Ed25519V1,
}

/// A name that is no key type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownKeyType {
    /// What was written.
    pub written: String,
}

impl fmt::Display for UnknownKeyType {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is no key type this server accepts", self.written)
    }
}

impl std::error::Error for UnknownKeyType {}

impl KeyType {
    /// The names a column and the wire store, and the type's byte in the key id.
    pub const NAMES: [(&'static str, Self); 1] = [("ed25519-v1", Self::Ed25519V1)];

    /// The name it is stored and written as.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ed25519V1 => "ed25519-v1",
        }
    }

    /// The byte that tells it apart inside a key id's input.
    const fn byte(self) -> u8 {
        match self {
            Self::Ed25519V1 => 1,
        }
    }
}

impl fmt::Display for KeyType {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for KeyType {
    type Err = UnknownKeyType;

    fn from_str(text: &str) -> Result<Self, UnknownKeyType> {
        Self::NAMES
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, found)| *found)
            .ok_or_else(|| UnknownKeyType { written: text.to_owned() })
    }
}

/// A key that cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyError {
    /// The 32 bytes are not a point on the curve.
    Point,
    /// The text is not the key text format.
    Text(Undecodable),
    /// The text does not start with `ink-ed25519-1:`.
    Prefix,
}

impl fmt::Display for KeyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Point => out.write_str("the public key is not a point of the curve"),
            Self::Text(_) => out.write_str("the key text is not 43 base64url characters"),
            Self::Prefix => write!(out, "the key text does not start with `{}`", KeyText::PREFIX),
        }
    }
}

impl std::error::Error for KeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Text(source) => Some(source),
            _ => None,
        }
    }
}

/// The private half of a key: 32 bytes that a machine made. It never prints, cannot be cloned
/// and is zeroed when dropped. The only way to see it is [`Seed::text`], once, for a person who
/// is allowed to keep it.
pub struct Seed([u8; 32]);

impl Seed {
    /// A seed from 32 bytes the caller drew.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// A seed from two draws of 16 bytes.
    pub fn from_draws(first: [u8; 16], second: [u8; 16]) -> Self {
        let mut bytes = [0_u8; 32];
        bytes.iter_mut().zip(first.iter().chain(second.iter())).for_each(|(into, from)| *into = *from);
        Self(bytes)
    }

    /// The bytes, for the signer.
    pub(crate) const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The seed as `ink-ed25519-1:<43 base64url characters>`: what a file or a CI secret holds.
    pub fn text(&self) -> KeyText {
        KeyText(format!("{}{}", KeyText::PREFIX, Base64::encode(&self.0)))
    }
}

impl fmt::Debug for Seed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Seed(<redacted>)")
    }
}

impl Drop for Seed {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl FromStr for Seed {
    type Err = KeyError;

    fn from_str(text: &str) -> Result<Self, KeyError> {
        let body = text.strip_prefix(KeyText::PREFIX).ok_or(KeyError::Prefix)?;
        Base64::decode::<32>(body).map(Self).map_err(KeyError::Text)
    }
}

/// A seed written out. It is text that holds a secret: it prints as redacted, and
/// [`KeyText::reveal`] is the one place the characters come out.
pub struct KeyText(String);

impl KeyText {
    /// The prefix every key text has, so a secret scanner can find one.
    pub const PREFIX: &'static str = "ink-ed25519-1:";

    /// The characters, for the file or the stream the person asked for.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for KeyText {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("KeyText(<redacted>)")
    }
}

impl Drop for KeyText {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// The public half of a key: 32 bytes the server stores and compares with.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    /// A public key from 32 bytes. Whether it is a point of the curve is checked when it is
    /// used to verify ([`crate::Verifying::new`]).
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The id of this key of `kind`: the first 16 bytes of a BLAKE3 key derivation over the
    /// type byte and the key, in the context that names the product and the year.
    pub fn id(&self, kind: KeyType) -> KeyId {
        let mut input = Vec::with_capacity(33);
        input.push(kind.byte());
        input.extend_from_slice(&self.0);
        let derived = blake3::derive_key(KeyId::CONTEXT, &input);
        let mut id = [0_u8; 16];
        id.iter_mut().zip(derived.iter()).for_each(|(into, from)| *into = *from);
        KeyId(id)
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "PublicKey({})", Base64::encode(&self.0))
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&Base64::encode(&self.0))
    }
}

impl FromStr for PublicKey {
    type Err = Undecodable;

    fn from_str(text: &str) -> Result<Self, Undecodable> {
        Base64::decode::<32>(text).map(Self)
    }
}

/// A key's id: 16 bytes derived from its public key, shown as 22 base64url characters. It is
/// public, and it is not chosen: it names the key and nothing else.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyId([u8; 16]);

impl KeyId {
    /// The BLAKE3 context. Changing it changes every key id, so a test holds its value.
    pub const CONTEXT: &'static str = "uses.ink 2026-11-27 key id v1";

    /// A key id from its 16 bytes, as a row stores it.
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// What a person compares: the first 10 characters in Crockford base32, `7QKM-2H4P-X9`.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint(Crockford::encode(&self.0, Fingerprint::CHARS))
    }
}

impl fmt::Debug for KeyId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "KeyId({})", self.fingerprint())
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&Base64::encode(&self.0))
    }
}

impl FromStr for KeyId {
    type Err = Undecodable;

    fn from_str(text: &str) -> Result<Self, Undecodable> {
        Base64::decode::<16>(text).map(Self)
    }
}

/// Text that is no fingerprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FingerprintError {
    /// What was written.
    pub written: String,
}

impl fmt::Display for FingerprintError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is not a key fingerprint like 7QKM-2H4P-X9", self.written)
    }
}

impl std::error::Error for FingerprintError {}

/// The first 10 characters of a key id in Crockford base32, grouped `XXXX-XXXX-XX`. It is
/// shown on both sides of an enrolment so a person can tell the key they made from one an
/// attacker made.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// How many characters it has.
    pub const CHARS: usize = 10;
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Fingerprint({self})")
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (a, rest) = self.0.split_at(4.min(self.0.len()));
        let (b, c) = rest.split_at(4.min(rest.len()));
        write!(out, "{a}-{b}-{c}")
    }
}

impl FromStr for Fingerprint {
    type Err = FingerprintError;

    /// Upper-case Crockford characters, with or without the dashes.
    fn from_str(text: &str) -> Result<Self, FingerprintError> {
        let bare: String = text.chars().filter(|c| *c != '-').collect();
        let valid = bare.len() == Self::CHARS && bare.bytes().all(|byte| Crockford::value(byte).is_some());
        if valid { Ok(Self(bare)) } else { Err(FingerprintError { written: text.to_owned() }) }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_seed_prints_nowhere_and_round_trips_as_text() {
        let seed = Seed::new([7; 32]);
        assert_eq!(format!("{seed:?}"), "Seed(<redacted>)");
        let text = seed.text();
        assert_eq!(format!("{text:?}"), "KeyText(<redacted>)");
        assert_eq!(text.reveal().len(), KeyText::PREFIX.len() + 43);
        assert_eq!(text.reveal().parse::<Seed>().map(|parsed| *parsed.bytes()), Ok([7; 32]));
        assert_eq!("ink-ed25519-2:AAAA".parse::<Seed>().err(), Some(KeyError::Prefix));
    }

    #[test]
    fn a_key_id_is_derived_and_its_context_is_held() {
        assert_eq!(KeyId::CONTEXT, "uses.ink 2026-11-27 key id v1");
        let key = PublicKey::new([9; 32]);
        let id = key.id(KeyType::Ed25519V1);
        assert_eq!(id, key.id(KeyType::Ed25519V1));
        assert_ne!(id, PublicKey::new([8; 32]).id(KeyType::Ed25519V1));
        assert_eq!(id.to_string().len(), 22);
        assert_eq!(id.to_string().parse::<KeyId>(), Ok(id));
        let print = id.fingerprint().to_string();
        assert_eq!(print.len(), 12);
        assert_eq!(print.parse::<Fingerprint>().map(|f| f.to_string()), Ok(print.clone()));
        assert!("7qkm-2h4p-x9".parse::<Fingerprint>().is_err());
    }

    #[test]
    fn key_types_are_named_once() {
        assert_eq!("ed25519-v1".parse::<KeyType>(), Ok(KeyType::Ed25519V1));
        assert!("rsa".parse::<KeyType>().is_err());
    }
}
