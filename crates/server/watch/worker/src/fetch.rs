//! The HTTP call.

use std::fmt;

/// One GET.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Get {
    /// The whole URL.
    pub url: String,
    /// The login to send, when the site has a credential. It redacts itself in `Debug`.
    pub basic: Option<Basic>,
}

/// What came back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// The status code, after redirects.
    pub status: u16,
    /// The body.
    pub body: Vec<u8>,
}

/// Makes a GET. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Worker's fetch is not Send")]
pub trait Fetch {
    /// The call's own error: it did not get an answer.
    type Error: std::error::Error + 'static;

    /// Gets `get`, following redirects.
    async fn get(&self, get: &Get) -> Result<Reply, Self::Error>;
}

/// `Basic <base64 of user:password>`. It prints as `Basic(<redacted>)`.
#[derive(Clone, PartialEq, Eq)]
pub struct Basic(String);

impl Basic {
    const ALPHABET: &'static [u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// The header value for `user` and `password`.
    pub fn new(user: &str, password: &str) -> Self {
        let text = format!("{user}:{password}");
        let mut out = String::from("Basic ");
        for chunk in text.as_bytes().chunks(3) {
            let word =
                chunk.iter().enumerate().fold(0_u32, |word, (at, byte)| word | (u32::from(*byte) << (16 - 8 * at)));
            for at in 0..4 {
                if at <= chunk.len() {
                    let index = usize::try_from((word >> (18 - 6 * at)) & 0x3f).unwrap_or(0);
                    out.push(char::from(Self::ALPHABET.get(index).copied().unwrap_or(b'=')));
                } else {
                    out.push('=');
                }
            }
        }
        Self(out)
    }

    /// The header value, for the one place that sends it.
    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Basic {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Basic(<redacted>)")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn encodes_as_rfc_4648_with_padding() {
        assert_eq!(Basic::new("git", "tok").value(), "Basic Z2l0OnRvaw==");
        assert_eq!(Basic::new("a", "b").value(), "Basic YTpi");
        assert_eq!(Basic::new("ab", "c").value(), "Basic YWI6Yw==");
        assert_eq!(Basic::new("user", "pass").value(), "Basic dXNlcjpwYXNz");
        assert_eq!(format!("{:?}", Basic::new("u", "p")), "Basic(<redacted>)");
    }
}
