//! Why a passkey ceremony was refused, as a value the caller can match on.

use std::error::Error;
use std::fmt;

/// What was wrong with a registration or an assertion. Each is a refusal of the ceremony and
/// changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Reason {
    /// The reply is not what the browser sends: a field is missing or does not parse.
    Malformed,
    /// The challenge is not the one this ceremony issued.
    Challenge,
    /// The page that asked is not one of the configured origins.
    Origin,
    /// The relying party id the authenticator signed for is not ours.
    RelyingParty,
    /// The authenticator did not verify the person (the user verification flag is clear).
    UserVerification,
    /// The signature does not verify under the stored key.
    Signature,
    /// The credential is not one of the account's, or is already registered.
    Credential,
    /// The signature counter went backwards on a hardware key.
    Counter,
    /// The credential's backup eligibility changed since it was registered.
    Backup,
    /// The credential's key is not ES256, the one algorithm the server takes.
    Algorithm,
}

impl Reason {
    /// A stable name, for logs and tests.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::Challenge => "challenge",
            Self::Origin => "origin",
            Self::RelyingParty => "relying-party",
            Self::UserVerification => "user-verification",
            Self::Signature => "signature",
            Self::Credential => "credential",
            Self::Counter => "counter",
            Self::Backup => "backup",
            Self::Algorithm => "algorithm",
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Malformed => "the reply is not a WebAuthn reply",
            Self::Challenge => "the challenge is not the one this ceremony issued",
            Self::Origin => "the page that asked is not one of the dashboard's origins",
            Self::RelyingParty => "the authenticator signed for another relying party id",
            Self::UserVerification => "the authenticator did not verify the person",
            Self::Signature => "the signature does not verify",
            Self::Credential => "the credential is not one this account may use",
            Self::Counter => "the signature counter went backwards on a hardware key",
            Self::Backup => "the credential's backup eligibility changed",
            Self::Algorithm => "the credential's key is not ES256",
        })
    }
}

/// A ceremony reply that was refused, with what the verifier said when it said anything.
#[derive(Debug)]
pub struct Refusal {
    /// What was wrong.
    pub reason: Reason,
    /// What the verifier said. Its error type belongs to the WebAuthn library the caller chose, and a
    /// generic would put it in every signature that can see a refusal: this is the one boxed edge.
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl Refusal {
    /// A refusal that has no more to say than its reason.
    pub const fn new(reason: Reason) -> Self {
        Self { reason, source: None }
    }

    /// A refusal because of what `source` said.
    pub fn because(reason: Reason, source: impl Error + Send + Sync + 'static) -> Self {
        Self { reason, source: Some(Box::new(source)) }
    }
}

impl From<Reason> for Refusal {
    fn from(reason: Reason) -> Self {
        Self::new(reason)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reason.fmt(out)
    }
}

impl Error for Refusal {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|source| source as &(dyn Error + 'static))
    }
}
