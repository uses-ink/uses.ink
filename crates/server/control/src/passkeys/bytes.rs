//! Bytes that travel as unpadded base64url text, and the id of a credential, which is such bytes.

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sig::Base64;

/// Bytes a browser and the server exchange as unpadded base64url (the form WebAuthn's JSON uses).
/// What decodes is exactly what encodes back to the same text.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Bytes(Vec<u8>);

impl Bytes {
    /// `bytes`, as they are.
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// The bytes, taken.
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for Bytes {
    /// Only how many: a public key or an assertion is not for a log line.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Bytes({} bytes)", self.0.len())
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&Base64::encode(&self.0))
    }
}

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&Base64::encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Base64::decode_vec(&text).map(Self).map_err(D::Error::custom)
    }
}

/// The id an authenticator gave a credential: up to 1023 bytes, unique across the credentials
/// of one relying party. It is public (the browser sends it), so it is no secret.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CredentialId(Bytes);

impl CredentialId {
    /// The longest id WebAuthn allows.
    pub const LONGEST: usize = 1023;

    /// An id from its bytes.
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(Bytes::new(bytes))
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl fmt::Display for CredentialId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bytes_are_base64url_text_and_never_print() {
        let bytes = Bytes::new(vec![0xfb, 0xff, 1]);
        assert_eq!(serde_json::to_string(&bytes).unwrap(), "\"-_8B\"");
        assert_eq!(serde_json::from_str::<Bytes>("\"-_8B\"").unwrap(), bytes);
        assert_eq!(format!("{bytes:?}"), "Bytes(3 bytes)");
        assert!(serde_json::from_str::<Bytes>("\"-_8=\"").is_err());
    }
}
