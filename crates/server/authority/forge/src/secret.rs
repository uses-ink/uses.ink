//! A credential that never prints: a webhook secret, an installation token, a bearer token.

use std::fmt;

use ring::{digest, hmac};
use subtle::ConstantTimeEq as _;
use zeroize::Zeroizing;

/// Bytes that authenticate something. It prints as `Secret(<redacted>)`, is zeroed when dropped, and
/// is neither `Clone`, `Display` nor `Serialize`. The only way to its bytes is [`Secret::reveal`],
/// which names every read of it.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    /// A secret of `bytes`.
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(Zeroizing::new(bytes.into()))
    }

    /// Its bytes, to put on the wire or to sign with. Nothing else should call this.
    pub fn reveal(&self) -> &[u8] {
        &self.0
    }

    /// Its text, when it is UTF-8: a token goes into a header as text.
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// Whether `given` is the same bytes, compared in time that does not depend on where they
    /// differ or on how long `given` is: both sides are hashed first, so only equal digests are
    /// compared and a guess learns nothing about the length of the secret.
    pub fn matches(&self, given: &[u8]) -> bool {
        let ours = digest::digest(&digest::SHA256, &self.0);
        let theirs = digest::digest(&digest::SHA256, given);
        ours.as_ref().ct_eq(theirs.as_ref()).into()
    }

    /// Whether `tag` is the HMAC-SHA256 of `body` under this secret, written as hex. The compare is
    /// ring's constant-time one, over the decoded bytes: a tag that is not 64 hex digits is refused
    /// before any compare, and the hex strings are never compared as text.
    pub fn authenticates(&self, body: &[u8], tag: &str) -> bool {
        let Some(tag) = Hex::decode(tag) else { return false };
        let key = hmac::Key::new(hmac::HMAC_SHA256, &self.0);
        tag.len() == digest::SHA256_OUTPUT_LEN && hmac::verify(&key, body, &tag).is_ok()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Secret(<redacted>)")
    }
}

/// Lower- or upper-case hex, two digits to a byte.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hex;

impl Hex {
    /// The bytes `text` spells, or none when it has an odd length or a character that is no digit.
    pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
        if !text.len().is_multiple_of(2) {
            return None;
        }
        text.as_bytes()
            .chunks(2)
            .map(|pair| {
                let high = char::from(*pair.first()?).to_digit(16)?;
                let low = char::from(*pair.get(1)?).to_digit(16)?;
                u8::try_from(high * 16 + low).ok()
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_secret_prints_nothing_and_hex_is_strict() {
        let secret = Secret::new(*b"hunter2-hunter2");
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert!(secret.matches(b"hunter2-hunter2"));
        assert!(!secret.matches(b"hunter2-hunter"));
        assert!(!secret.matches(b""));
        assert_eq!(Hex::decode("0aFf"), Some(vec![0x0a, 0xff]));
        assert_eq!(Hex::decode("0af"), None);
        assert_eq!(Hex::decode("0g"), None);
        assert_eq!(Hex::decode("é1"), None);
    }
}
