//! Secrets that prove something, and the digests that are all that is kept of them.

use std::fmt;
use std::str::FromStr;

use secret::Plain;

use control_rows::Digest;

use control_rows::id::{Account as AccountKind, Factor as FactorKind, Id, Session as SessionKind};

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
    /// The secret in the cookie of a passkey ceremony.
    Ceremony,
}

impl Context {
    /// The digest of `proof` for this context.
    pub fn digest(self, proof: &Proof) -> Digest {
        Digest::derive(self.key(), proof.reveal().as_bytes())
    }

    const fn key(self) -> &'static str {
        match self {
            Self::Challenge => "uses.ink 2026-11-27 sign-in challenge",
            Self::Session => "uses.ink 2026-11-27 sign-in session",
            Self::Ceremony => "uses.ink 2026-11-27 passkey ceremony",
        }
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
    use control_rows::NotDigest;

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
        let one = Context::Challenge.digest(&proof);
        assert_eq!(one, Context::Challenge.digest(&proof));
        assert_ne!(one, Context::Session.digest(&proof));
        assert_ne!(one, Context::Ceremony.digest(&proof));
        assert_ne!(Context::Session.digest(&proof), Context::Ceremony.digest(&proof));
        assert_eq!(Digest::try_from(&one.as_bytes()[..]).unwrap(), one);
        assert_eq!(Digest::try_from(&[1_u8; 5][..]), Err(NotDigest));
    }
}
