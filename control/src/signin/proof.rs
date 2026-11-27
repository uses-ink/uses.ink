//! Secrets that prove something, and the digests that are all that is kept of them.

use std::fmt;
use std::str::FromStr;

use secret::Plain;

use crate::id::{Account as AccountKind, Factor as FactorKind, Id, Session as SessionKind};

/// A random 256-bit secret as 64 lower-case hex digits: a one-time sign-in token, or the secret
/// of a session. It prints as `Proof(<redacted>)`, is zeroed when dropped, and cannot be cloned
/// or serialized; the only way to its text is [`Proof::reveal`], which names every read of it.
pub struct Proof(Plain);

impl Proof {
    /// How many hex digits a proof has.
    pub const DIGITS: usize = 64;

    /// A proof from 32 bytes the caller drew.
    pub fn from_bytes(bytes: [[u8; 16]; 2]) -> Self {
        let digits: String = bytes
            .iter()
            .flatten()
            .flat_map(|byte| [byte >> 4, byte & 15])
            .filter_map(|digit| char::from_digit(u32::from(digit), 16))
            .collect();
        Self(Plain::from(digits.as_str()))
    }

    /// The proof's text, to put in a link or a cookie.
    pub fn reveal(&self) -> &str {
        std::str::from_utf8(self.0.expose()).unwrap_or_default()
    }
}

impl fmt::Debug for Proof {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Proof(<redacted>)")
    }
}

/// Text that is no proof. It says how it differs in shape and never what it held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProofError {
    /// Not 64 characters long.
    Length {
        /// How long it is.
        found: usize,
    },
    /// A character that is not a lower-case hex digit.
    Digit,
}

impl fmt::Display for ProofError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a token is {} characters, not {found}", Proof::DIGITS),
            Self::Digit => out.write_str("a token is lower-case hex digits"),
        }
    }
}

impl std::error::Error for ProofError {}

impl FromStr for Proof {
    type Err = ProofError;

    fn from_str(text: &str) -> Result<Self, ProofError> {
        if text.len() != Self::DIGITS {
            return Err(ProofError::Length { found: text.len() });
        }
        if !text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return Err(ProofError::Digit);
        }
        Ok(Self(Plain::from(text)))
    }
}

/// What a digest was made for. Each has its own key, so a token that is a valid sign-in link
/// is no valid session, and the reverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Context {
    /// A one-time token.
    Challenge,
    /// A session's secret.
    Session,
}

impl Context {
    const fn key(self) -> &'static str {
        match self {
            Self::Challenge => "uses.ink 2026-11-27 sign-in challenge",
            Self::Session => "uses.ink 2026-11-27 sign-in session",
        }
    }
}

/// The BLAKE3 digest of a proof under a [`Context`]. Proofs are 256 random bits, so a digest of
/// one cannot be reversed or guessed at, and no salt or stretching is needed. Two digests are
/// compared in constant time.
#[derive(Clone, Copy)]
pub struct Digest(blake3::Hash);

impl Digest {
    /// The digest of `proof` for `context`.
    pub fn of(context: Context, proof: &Proof) -> Self {
        Self(blake3::Hash::from(blake3::derive_key(context.key(), proof.reveal().as_bytes())))
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

/// Everything one sign-in call may need, drawn by the caller before it is made: the secret of
/// the link or session it makes, and the ids of the rows it may add. The same calls with the
/// same draws make the same rows in a test.
#[derive(Debug)]
pub struct Drawn {
    /// The secret of the link or the session the call makes.
    pub proof: Proof,
    /// The id of a session it may add.
    pub session: Id<SessionKind>,
    /// The id of an account it may add.
    pub account: Id<AccountKind>,
    /// The id of a factor it may add.
    pub factor: Id<FactorKind>,
}

impl Drawn {
    /// Five draws of 16 bytes: two for the proof, one for each id.
    pub fn new(bytes: [[u8; 16]; 5]) -> Self {
        let [a, b, session, account, factor] = bytes;
        Self {
            proof: Proof::from_bytes([a, b]),
            session: Id::from_bytes(session),
            account: Id::from_bytes(account),
            factor: Id::from_bytes(factor),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_proof_is_64_hex_digits_and_never_prints() {
        let proof = Proof::from_bytes([[0xab; 16], [0x01; 16]]);
        assert_eq!(proof.reveal().len(), Proof::DIGITS);
        assert!(proof.reveal().starts_with("abab"));
        assert_eq!(format!("{proof:?}"), "Proof(<redacted>)");
        assert_eq!(proof.reveal().parse::<Proof>().unwrap().reveal(), proof.reveal());
    }

    #[test]
    fn text_that_is_no_proof_says_how_it_differs() {
        assert_eq!("abc".parse::<Proof>().unwrap_err(), ProofError::Length { found: 3 });
        assert_eq!("g".repeat(64).parse::<Proof>().unwrap_err(), ProofError::Digit);
        assert_eq!("A".repeat(64).parse::<Proof>().unwrap_err(), ProofError::Digit);
    }

    #[test]
    fn a_digest_is_keyed_by_what_it_is_for() {
        let proof = Proof::from_bytes([[7; 16], [8; 16]]);
        let one = Digest::of(Context::Challenge, &proof);
        assert_eq!(one, Digest::of(Context::Challenge, &proof));
        assert_ne!(one, Digest::of(Context::Session, &proof));
        assert_eq!(Digest::try_from(&one.as_bytes()[..]).unwrap(), one);
        assert_eq!(Digest::try_from(&[1_u8; 5][..]), Err(NotDigest));
    }
}
