//! The App's private key, and what signs with it.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ring::error::{KeyRejected, Unspecified};
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use zeroize::Zeroizing;

use authority_forge::Secret;

/// Signs a message with `RSASSA-PKCS1-v1_5` and SHA-256, which is RS256. A Worker implements it over
/// `WebCrypto`; [`Key`] implements it over `ring`.
pub trait Signer {
    /// What signing fails with.
    type Error: std::error::Error + 'static;

    /// The signature of `message`.
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

/// What a PEM label that is not an RSA private key's names. A label this crate does not know is
/// counted and never kept: a pasted key can have its body on the first line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Armor {
    /// `CERTIFICATE`.
    Certificate,
    /// `PUBLIC KEY` or `RSA PUBLIC KEY`.
    Public,
    /// `EC PRIVATE KEY`.
    Curve,
    /// `ENCRYPTED PRIVATE KEY`.
    Encrypted,
    /// `OPENSSH PRIVATE KEY`.
    Openssh,
    /// Any other label.
    Other {
        /// How many bytes it had.
        length: usize,
    },
}

impl Armor {
    /// What `label` names.
    fn of(label: &str) -> Self {
        match label {
            "CERTIFICATE" => Self::Certificate,
            "PUBLIC KEY" | "RSA PUBLIC KEY" => Self::Public,
            "EC PRIVATE KEY" => Self::Curve,
            "ENCRYPTED PRIVATE KEY" => Self::Encrypted,
            "OPENSSH PRIVATE KEY" => Self::Openssh,
            other => Self::Other { length: other.len() },
        }
    }
}

impl fmt::Display for Armor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Certificate => out.write_str("a certificate"),
            Self::Public => out.write_str("a public key"),
            Self::Curve => out.write_str("an elliptic curve key"),
            Self::Encrypted => out.write_str("an encrypted key"),
            Self::Openssh => out.write_str("an OpenSSH key"),
            Self::Other { length } => write!(out, "unknown, {length} bytes"),
        }
    }
}

/// A private key that could not be read.
#[derive(Debug)]
#[non_exhaustive]
pub enum KeyError {
    /// The key is not UTF-8 text.
    Text,
    /// There is no `-----BEGIN` line, or no matching `-----END`.
    Frame,
    /// The PEM label is neither `RSA PRIVATE KEY` (PKCS#1, what GitHub gives) nor `PRIVATE KEY`
    /// (PKCS#8).
    Label {
        /// What the label is, without the label: it is the one line of a key that may hold more
        /// than a name, so it is never echoed.
        armor: Armor,
    },
    /// The body is not base64.
    Base64(base64::DecodeError),
    /// The key is no RSA key `ring` accepts (it needs 2048 bits or more).
    Rejected(KeyRejected),
}

impl KeyError {
    /// The code of the catalog that words this.
    pub const fn code(&self) -> &'static str {
        "app.key.invalid"
    }
}

impl fmt::Display for KeyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => out.write_str("the App key is not text"),
            Self::Frame => out.write_str("the App key has no PEM begin and end lines"),
            Self::Label { armor } => write!(out, "the PEM label ({armor}) is not an RSA private key"),
            Self::Base64(_) => out.write_str("the App key's body is not base64"),
            Self::Rejected(_) => out.write_str("the App key is not an RSA key of 2048 bits or more"),
        }
    }
}

impl std::error::Error for KeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Base64(source) => Some(source),
            Self::Rejected(source) => Some(source),
            Self::Text | Self::Frame | Self::Label { .. } => None,
        }
    }
}

/// The App's RSA private key. It prints as `Key(<redacted>)` and holds no PEM text.
pub struct Key(RsaKeyPair);

impl Key {
    /// Reads a PEM private key: PKCS#1 (`BEGIN RSA PRIVATE KEY`, as GitHub issues it) or PKCS#8.
    pub fn read(pem: &Secret) -> Result<Self, KeyError> {
        let text = pem.text().ok_or(KeyError::Text)?;
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
        let label = lines
            .next()
            .and_then(|line| line.strip_prefix("-----BEGIN ")?.strip_suffix("-----"))
            .ok_or(KeyError::Frame)?;
        let end = format!("-----END {label}-----");
        let mut body = Zeroizing::new(String::new());
        for line in lines.by_ref() {
            if line == end {
                let der = Zeroizing::new(STANDARD.decode(body.as_bytes()).map_err(KeyError::Base64)?);
                return Self::pair(label, &der);
            }
            body.push_str(line);
        }
        Err(KeyError::Frame)
    }

    fn pair(label: &str, der: &[u8]) -> Result<Self, KeyError> {
        let pair = match label {
            "RSA PRIVATE KEY" => RsaKeyPair::from_der(der),
            "PRIVATE KEY" => RsaKeyPair::from_pkcs8(der),
            other => return Err(KeyError::Label { armor: Armor::of(other) }),
        };
        pair.map(Self).map_err(KeyError::Rejected)
    }

    /// The public key as PKCS#1 DER, which is what verifies a signature.
    pub fn public(&self) -> &[u8] {
        self.0.public().as_ref()
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Key(<redacted>)")
    }
}

impl Signer for Key {
    type Error = Unspecified;

    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, Unspecified> {
        let mut signature = vec![0; self.0.public().modulus_len()];
        self.0.sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), message, &mut signature)?;
        Ok(signature)
    }
}
