//! The URL a message carries: a base that is configured, and a token that is secret.

use std::fmt;
use std::str::FromStr;

/// Where links point: an `http` or `https` URL with a host and no query or fragment. The token
/// is appended as one more path segment, so a base of `https://ink.example/login` makes
/// `https://ink.example/login/<token>`. Whatever serves that page decides what it does with the
/// token; the API's own pages are under `/v1/login`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base(String);

/// Text that is no base.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BaseError {
    /// The scheme is not `http` or `https`.
    Scheme,
    /// There is no host after the scheme.
    Host,
    /// A query, a fragment, white space or a control character, at this byte offset.
    Character {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for BaseError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheme => out.write_str("a link base starts with http:// or https://"),
            Self::Host => out.write_str("a link base needs a host"),
            Self::Character { at } => {
                write!(out, "byte {at} of the link base is not allowed: no query, fragment or space")
            }
        }
    }
}

impl std::error::Error for BaseError {}

impl FromStr for Base {
    type Err = BaseError;

    fn from_str(text: &str) -> Result<Self, BaseError> {
        let rest = text.strip_prefix("https://").or_else(|| text.strip_prefix("http://")).ok_or(BaseError::Scheme)?;
        if rest.is_empty() || rest.starts_with('/') {
            return Err(BaseError::Host);
        }
        if let Some(at) = text.bytes().position(|b| b == b'?' || b == b'#' || !b.is_ascii_graphic()) {
            return Err(BaseError::Character { at });
        }
        Ok(Self(text.trim_end_matches('/').to_owned()))
    }
}

impl fmt::Display for Base {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl Base {
    /// The link for `token`. The token is a path segment, so it must be made of URL-safe
    /// characters; the control plane's tokens are lower-case hex.
    pub fn link(&self, token: &str) -> Link {
        Link(format!("{}/{token}", self.0))
    }
}

/// A link that signs someone in. Its text is a secret until it is used: it prints as
/// `Link(<redacted>)` in `Debug`, and the only way to its text is `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct Link(String);

impl fmt::Debug for Link {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Link(<redacted>)")
    }
}

impl fmt::Display for Link {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_base_takes_the_token_as_its_last_segment() {
        let base: Base = "https://ink.example/login/".parse().unwrap();
        assert_eq!(base.link("abc").to_string(), "https://ink.example/login/abc");
        assert_eq!(format!("{:?}", base.link("abc")), "Link(<redacted>)");
    }

    #[test]
    fn a_base_is_a_plain_http_url() {
        assert_eq!("ftp://x".parse::<Base>(), Err(BaseError::Scheme));
        assert_eq!("https://".parse::<Base>(), Err(BaseError::Host));
        assert_eq!("https://x/a?b=1".parse::<Base>(), Err(BaseError::Character { at: 11 }));
        assert_eq!("https://x/a b".parse::<Base>(), Err(BaseError::Character { at: 11 }));
    }
}
