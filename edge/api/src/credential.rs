//! Who is calling: a bearer token, checked through a port.
//!
//! A [`Credentials`] adapter says which account a token belongs to. [`Owner`] knows one token,
//! the operator's, for a self-host with one person. [`Sessions`] knows the sessions that signing
//! in makes (decision 0066). [`Either`] asks one adapter and then another, so a new kind of
//! credential (a passkey's signed request, a device key) is a new adapter put in an `Either`
//! where the API is built, and no caller changes.

use std::error::Error;
use std::fmt;
use std::future::Future;

use clock::{Clock, Draw};
use control::signin::{Proof, SignIn};
use control::{AccountKind, Id};
use db::Db;

/// The longest token the API reads.
pub const LONGEST: usize = 256;

/// A bearer token as sent: `Authorization: Bearer <token>`. It never prints, so a log line or a
/// debug dump cannot leak it.
#[derive(Clone, PartialEq, Eq)]
pub struct Bearer(String);

impl Bearer {
    /// The token's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Bearer {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Bearer(..)")
    }
}

/// An `Authorization` value that is no bearer token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BearerError {
    /// Not text.
    Text,
    /// The scheme is not `Bearer`.
    Scheme,
    /// Nothing after the scheme.
    Empty,
    /// More than [`LONGEST`] bytes.
    Long {
        /// How long.
        found: usize,
    },
    /// A byte that is not visible ASCII, at this offset.
    Character {
        /// The offset in the token.
        at: usize,
    },
}

impl fmt::Display for BearerError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => out.write_str("the Authorization header is not text"),
            Self::Scheme => out.write_str("the Authorization header is not a Bearer token"),
            Self::Empty => out.write_str("the Bearer token is empty"),
            Self::Long { found } => write!(out, "the Bearer token is {found} bytes, over {LONGEST}"),
            Self::Character { at } => write!(out, "byte {at} of the Bearer token is not visible ASCII"),
        }
    }
}

impl Error for BearerError {}

impl TryFrom<&[u8]> for Bearer {
    type Error = BearerError;

    fn try_from(header: &[u8]) -> Result<Self, BearerError> {
        let text = std::str::from_utf8(header).map_err(|_| BearerError::Text)?;
        let (scheme, token) = text.split_once(' ').unwrap_or((text, ""));
        if !scheme.eq_ignore_ascii_case("bearer") {
            return Err(BearerError::Scheme);
        }
        if token.is_empty() {
            return Err(BearerError::Empty);
        }
        if token.len() > LONGEST {
            return Err(BearerError::Long { found: token.len() });
        }
        match token.bytes().position(|byte| !byte.is_ascii_graphic()) {
            Some(at) => Err(BearerError::Character { at }),
            None => Ok(Self(token.to_owned())),
        }
    }
}

/// Who a token speaks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Principal {
    /// The account.
    pub account: Id<AccountKind>,
}

/// Which account a token belongs to. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a session store on Workers has futures that are not Send")]
pub trait Credentials {
    /// Why the check could not be made.
    type Error: Error + 'static;

    /// The account `bearer` speaks for, or `None` when it speaks for nobody.
    async fn verify(&self, bearer: &Bearer) -> Result<Option<Principal>, Self::Error>;
}

/// A token an [`Owner`] is made from: 64 hex digits of two draws.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// The token as text, to show its owner once.
    pub fn reveal(&self) -> &str {
        &self.0
    }

    /// A fresh token from two draws.
    pub fn drawn<D: Draw>(draw: &D) -> Result<Self, D::Error> {
        let bytes = [draw.bytes()?, draw.bytes()?].concat();
        let digits = bytes.iter().flat_map(|byte| [byte >> 4, byte & 15]);
        Ok(Self(digits.filter_map(|digit| char::from_digit(u32::from(digit), 16)).collect()))
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Token(..)")
    }
}

impl From<&str> for Token {
    fn from(text: &str) -> Self {
        Self(text.to_owned())
    }
}

/// One token and the account it speaks for: a self-host with one owner. Only the token's BLAKE3
/// digest is kept, and the comparison of digests takes the same time wherever they differ.
#[derive(Clone, Debug)]
pub struct Owner {
    digest: blake3::Hash,
    account: Id<AccountKind>,
}

impl Owner {
    /// The owner of `account`, who holds `token`.
    pub fn new(account: Id<AccountKind>, token: &Token) -> Self {
        Self { digest: blake3::hash(token.0.as_bytes()), account }
    }
}

impl Credentials for Owner {
    type Error = std::convert::Infallible;

    fn verify(&self, bearer: &Bearer) -> impl Future<Output = Result<Option<Principal>, Self::Error>> {
        let presented = blake3::hash(bearer.as_str().as_bytes());
        std::future::ready(Ok((presented == self.digest).then_some(Principal { account: self.account })))
    }
}

/// The sessions signing in makes: a bearer token is a session's secret, and speaks for the
/// account of that session while it has not ended or been revoked. Only a digest of the secret
/// is looked up, and a sliding session is moved on by the check.
#[derive(Clone, Debug)]
pub struct Sessions<D, K> {
    entry: SignIn<D>,
    clock: K,
}

impl<D: Db, K: Clock> Sessions<D, K> {
    /// The sessions of `entry`, ending by `clock`.
    pub const fn new(entry: SignIn<D>, clock: K) -> Self {
        Self { entry, clock }
    }
}

impl<D: Db, K: Clock> Credentials for Sessions<D, K> {
    type Error = control::signin::Error<D::Error>;

    async fn verify(&self, bearer: &Bearer) -> Result<Option<Principal>, Self::Error> {
        let Ok(proof) = bearer.as_str().parse::<Proof>() else { return Ok(None) };
        let session = self.entry.resolve(&proof, self.clock.now()).await?;
        Ok(session.map(|session| Principal { account: session.account }))
    }
}

/// Two adapters, asked in turn: the first that knows the token answers.
#[derive(Clone, Debug)]
pub struct Either<A, B>(pub A, pub B);

/// Which adapter of an [`Either`] failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum Which<A, B> {
    /// The first.
    First(A),
    /// The second.
    Second(B),
}

impl<A, B> fmt::Display for Which<A, B> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::First(_) => out.write_str("the first credential check failed"),
            Self::Second(_) => out.write_str("the second credential check failed"),
        }
    }
}

impl<A: Error + 'static, B: Error + 'static> Error for Which<A, B> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::First(source) => Some(source),
            Self::Second(source) => Some(source),
        }
    }
}

impl<A: Credentials, B: Credentials> Credentials for Either<A, B> {
    type Error = Which<A::Error, B::Error>;

    async fn verify(&self, bearer: &Bearer) -> Result<Option<Principal>, Self::Error> {
        match self.0.verify(bearer).await.map_err(Which::First)? {
            Some(principal) => Ok(Some(principal)),
            None => self.1.verify(bearer).await.map_err(Which::Second),
        }
    }
}

#[cfg(test)]
mod test {
    use clock::Counter;

    use super::*;

    #[test]
    fn reads_a_bearer_header_and_nothing_else() {
        let read = |text: &str| Bearer::try_from(text.as_bytes());
        assert_eq!(read("Bearer abc").unwrap().as_str(), "abc");
        assert_eq!(read("bearer abc").unwrap().as_str(), "abc");
        assert_eq!(read("Basic abc"), Err(BearerError::Scheme));
        assert_eq!(read("Bearer "), Err(BearerError::Empty));
        assert_eq!(read("Bearer"), Err(BearerError::Empty));
        assert_eq!(read("Bearer a b"), Err(BearerError::Character { at: 1 }));
        assert_eq!(Bearer::try_from(&[0xff][..]), Err(BearerError::Text));
        assert_eq!(read(&format!("Bearer {}", "a".repeat(300))), Err(BearerError::Long { found: 300 }));
        assert_eq!(format!("{:?}", read("Bearer secret").unwrap()), "Bearer(..)");
    }

    #[test]
    fn the_owner_token_speaks_for_the_owner_and_no_other_does() {
        let account = Id::from_bytes([7; 16]);
        let token = Token::drawn(&Counter::new()).unwrap();
        assert_eq!(token.reveal().len(), 64);
        let owner = Owner::new(account, &token);
        let bearer = Bearer::try_from(format!("Bearer {}", token.reveal()).as_bytes()).unwrap();
        assert_eq!(pollster::block_on(owner.verify(&bearer)), Ok(Some(Principal { account })));
        let other = Bearer::try_from(&b"Bearer other"[..]).unwrap();
        assert_eq!(pollster::block_on(owner.verify(&other)), Ok(None));
    }
}
