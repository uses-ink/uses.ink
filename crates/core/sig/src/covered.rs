//! The signed text: what the signature is over.

use std::fmt;

use crate::alphabet::Base64;
use crate::signed::Claim;
use crate::target::Target;

/// The authority a request is for, as the server is configured: lower-case letters, digits, `.`,
/// `-` and `:`. A signed request to one instance fails on another.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Audience(String);

/// Text that is no authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudienceError {
    /// Nothing.
    Empty,
    /// A character the authority alphabet does not have, at this offset.
    Character {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for AudienceError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("the audience is empty"),
            Self::Character { at } => write!(out, "byte {at} of the audience is not a letter, digit, `.`, `-` or `:`"),
        }
    }
}

impl std::error::Error for AudienceError {}

impl Audience {
    /// `text` as an authority; upper-case letters are made lower case.
    pub fn parse(text: &str) -> Result<Self, AudienceError> {
        if text.is_empty() {
            return Err(AudienceError::Empty);
        }
        let lower = text.to_ascii_lowercase();
        match lower
            .bytes()
            .position(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-:".contains(&byte)))
        {
            Some(at) => Err(AudienceError::Character { at }),
            None => Ok(Self(lower)),
        }
    }

    /// The authority.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Audience {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// An HTTP method as the signed text writes it: upper-case ASCII letters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Method(String);

/// Text that is no method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodError {
    /// What was written.
    pub written: String,
}

impl fmt::Display for MethodError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is not an upper-case method name", self.written)
    }
}

impl std::error::Error for MethodError {}

impl Method {
    /// `text` as a method.
    pub fn parse(text: &str) -> Result<Self, MethodError> {
        if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_uppercase()) {
            Ok(Self(text.to_owned()))
        } else {
            Err(MethodError { written: text.to_owned() })
        }
    }

    /// The method.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Method {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// What the signature is over: a context line, then one labelled line each for the audience,
/// the method, the target, the digest, the time, the nonce and the key id, every line ending in
/// `\n`. No value's alphabet holds `\n`, every line has a fixed label and the order is fixed, so
/// there is one parse of the text. The first line names the product, the protocol and its
/// version, so a signature made here verifies nowhere else and nothing made elsewhere verifies
/// here. The key id is signed, so a signature cannot be moved to another key.
#[derive(Clone, Copy, Debug)]
pub struct Covered<'a> {
    /// The authority the server is configured with.
    pub audience: &'a Audience,
    /// The method.
    pub method: &'a Method,
    /// The canonical target.
    pub target: &'a Target,
    /// The key, the time, the nonce and the body's digest.
    pub claim: &'a Claim,
}

impl Covered<'_> {
    /// The first line.
    pub const CONTEXT: &'static str = "uses.ink ink-sig 1";

    /// The text that is signed, as bytes.
    pub fn bytes(&self) -> Vec<u8> {
        let claim = self.claim;
        format!(
            "{}\naud {}\nmethod {}\ntarget {}\ndigest {}\ncreated {}\nnonce {}\nkey {}\n",
            Self::CONTEXT,
            self.audience,
            self.method,
            self.target,
            Base64::encode(claim.digest.bytes()),
            claim.created,
            claim.nonce,
            claim.key,
        )
        .into_bytes()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::key::KeyId;
    use crate::signed::{Created, Digest, Nonce};

    #[test]
    fn the_text_has_eight_labelled_lines_and_no_value_can_add_one() {
        let claim = Claim {
            key: KeyId::new([1; 16]),
            created: Created::new(1_795_000_000),
            nonce: Nonce::new([2; 16]),
            digest: Digest::of(b""),
        };
        let audience = Audience::parse("API.Uses.Ink:8080").unwrap();
        let method = Method::parse("GET").unwrap();
        let target = Target::parse("/v1/sites?a=1").unwrap();
        let text =
            String::from_utf8(Covered { audience: &audience, method: &method, target: &target, claim: &claim }.bytes())
                .unwrap();
        let labels: Vec<&str> = text.lines().map(|line| line.split(' ').next().unwrap_or_default()).collect();
        assert_eq!(labels, ["uses.ink", "aud", "method", "target", "digest", "created", "nonce", "key"]);
        assert!(text.ends_with('\n'));
        assert!(
            text.starts_with("uses.ink ink-sig 1\naud api.uses.ink:8080\nmethod GET\ntarget /v1/sites?a=1\ndigest ")
        );
        assert!(Audience::parse("a\nb").is_err());
        assert!(Audience::parse("a b").is_err());
        assert!(Audience::parse("").is_err());
        assert!(Method::parse("get").is_err());
        assert!(Method::parse("GE\nT").is_err());
    }
}
