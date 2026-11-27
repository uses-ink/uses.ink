//! The ceremonies as values: what the server asks the browser, what the browser answers, and
//! what a verified answer says. The JSON field names are the ones WebAuthn's options use, so the
//! browser script passes them to `navigator.credentials` after turning each base64url string
//! into bytes.

use serde::{Deserialize, Serialize};

use super::{Bytes, CredentialId};
use crate::id::{Account as AccountKind, Id};
use crate::keys::Label;

/// Who a passkey is registered for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    /// The account. Its 16 bytes are the WebAuthn user handle: random, and no personal data.
    pub account: Id<AccountKind>,
    /// The name the authenticator shows for the passkey.
    pub label: Label,
}

/// ES256, the one algorithm the server takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Algorithm {
    /// Always `public-key`.
    #[serde(rename = "type")]
    pub kind: Class,
    /// The COSE algorithm number: -7 is ES256.
    pub alg: i32,
}

impl Algorithm {
    /// ECDSA over P-256 with SHA-256.
    pub const ES256: Self = Self { kind: Class::PublicKey, alg: -7 };
}

/// The one kind of credential there is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Class {
    /// `public-key`.
    #[serde(rename = "public-key")]
    PublicKey,
}

/// An existing credential, named so the browser skips it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Named {
    /// Always `public-key`.
    #[serde(rename = "type")]
    pub kind: Class,
    /// The credential.
    pub id: CredentialId,
}

impl From<CredentialId> for Named {
    fn from(id: CredentialId) -> Self {
        Self { kind: Class::PublicKey, id }
    }
}

/// A requirement the server sets and checks again on the reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Requirement {
    /// Must be.
    Required,
}

/// What the server asks of the authenticator at registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    /// A discoverable credential: the sign-in asks for no username.
    pub resident_key: Requirement,
    /// The person is verified by a PIN, a biometric or the like.
    pub user_verification: Requirement,
}

/// Attestation is not asked for: no filter on makes of authenticator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Attestation {
    /// `none`.
    None,
}

/// The relying party, as the browser is told.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Party {
    /// The relying party id.
    pub id: String,
    /// The name shown.
    pub name: String,
}

/// The person, as the browser is told.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
    /// The user handle: the account's 16 bytes.
    pub id: Bytes,
    /// The passkey's name.
    pub name: String,
    /// The same, for display.
    pub display_name: String,
}

/// The options of `navigator.credentials.create()`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Creation {
    /// The relying party.
    pub rp: Party,
    /// The person.
    pub user: Holder,
    /// 32 random bytes, used once.
    pub challenge: Bytes,
    /// Only ES256.
    pub pub_key_cred_params: Vec<Algorithm>,
    /// How long the browser may wait, in milliseconds.
    pub timeout: u32,
    /// The account's passkeys, so one authenticator makes one passkey.
    pub exclude_credentials: Vec<Named>,
    /// Discoverable, user verifying.
    pub authenticator_selection: Selection,
    /// `none`.
    pub attestation: Attestation,
}

/// The options of `navigator.credentials.get()` for a sign-in with no username: no credential is
/// named, so the authenticator offers the passkeys it holds for this relying party.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asking {
    /// 32 random bytes, used once.
    pub challenge: Bytes,
    /// The relying party id.
    pub rp_id: String,
    /// How long the browser may wait, in milliseconds.
    pub timeout: u32,
    /// Required.
    pub user_verification: Requirement,
    /// Empty: a discoverable credential.
    pub allow_credentials: Vec<Named>,
}

/// How an authenticator reaches the device. A hint for the browser, never a security property.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// USB.
    Usb,
    /// NFC.
    Nfc,
    /// Bluetooth.
    Ble,
    /// The platform's own.
    Internal,
    /// A phone nearby.
    Hybrid,
    /// A transport this version does not know. It is kept as `unknown`, so nothing is dropped.
    #[serde(other)]
    Unknown,
}

impl Transport {
    /// The names a column stores, in the order they are listed.
    pub const NAMES: [(&'static str, Self); 6] = [
        ("usb", Self::Usb),
        ("nfc", Self::Nfc),
        ("ble", Self::Ble),
        ("internal", Self::Internal),
        ("hybrid", Self::Hybrid),
        ("unknown", Self::Unknown),
    ];

    /// The name it is stored as.
    pub fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, found)| *found == self).map_or("unknown", |(name, _)| name)
    }

    /// The transport a stored name is, `Unknown` for a name no version wrote.
    pub fn named(text: &str) -> Self {
        Self::NAMES.iter().find(|(name, _)| *name == text).map_or(Self::Unknown, |(_, found)| *found)
    }
}

/// What the browser answers to a registration: the public-key credential's parts.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Created {
    /// The credential's id as the browser has it.
    pub id: CredentialId,
    /// `clientDataJSON`.
    pub client_data: Bytes,
    /// The attestation object, CBOR.
    pub attestation: Bytes,
    /// The transports the authenticator reported.
    #[serde(default)]
    pub transports: Vec<Transport>,
}

/// What the browser answers to a sign-in.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asserted {
    /// The credential's id.
    pub id: CredentialId,
    /// `clientDataJSON`.
    pub client_data: Bytes,
    /// The authenticator data.
    pub authenticator: Bytes,
    /// The signature, DER.
    pub signature: Bytes,
    /// The user handle: the account's 16 bytes. A discoverable credential always returns it.
    #[serde(default)]
    pub handle: Option<Bytes>,
}

/// What a registration was before it was answered: the server's side, kept until the browser
/// answers or the ceremony ends. Opaque: the adapter that made it reads it back.
#[derive(Clone, PartialEq, Eq)]
pub struct State(Vec<u8>);

impl State {
    /// A state from the adapter's bytes.
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "State({} bytes)", self.0.len())
    }
}

/// A registration that was started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registering(pub State);

/// A sign-in that was started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authenticating(pub State);

/// A credential as the server keeps it: what a later sign-in is verified against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    /// The credential's id.
    pub credential: CredentialId,
    /// The public key, in the adapter's own encoding.
    pub key: Bytes,
    /// The signature counter last seen; 0 when the authenticator keeps none.
    pub counter: u32,
    /// Whether the credential may be synced or backed up.
    pub eligible: bool,
    /// Whether it is backed up now.
    pub backed_up: bool,
    /// The transports it reported.
    pub transports: Vec<Transport>,
}

/// A registration that verified.
pub type Enrolled = Stored;

/// Which credential an assertion says it is for, read before it is verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    /// The credential.
    pub credential: CredentialId,
    /// The account its user handle names.
    pub account: Id<AccountKind>,
}

/// What the signature counter said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Counter {
    /// Neither the stored nor the presented counter is above zero: a synced passkey, which keeps none.
    Unused,
    /// The counter rose.
    Rose,
    /// It did not rise, on a credential that may be synced. Several devices may share a value,
    /// so it is flagged and the sign-in goes on.
    Flagged,
    /// It did not rise, on a hardware key. Hardware keys do not run backwards unless cloned.
    Refused,
}

impl Counter {
    /// The policy of `docs/auth.md` section 4. A counter is used only when it is used: if either
    /// the stored or the presented value is above zero, it must rise.
    pub const fn judge(stored: u32, presented: u32, eligible: bool) -> Self {
        if stored == 0 && presented == 0 {
            Self::Unused
        } else if presented > stored {
            Self::Rose
        } else if eligible {
            Self::Flagged
        } else {
            Self::Refused
        }
    }
}

/// A sign-in that verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    /// The credential.
    pub credential: CredentialId,
    /// The counter to store.
    pub counter: u32,
    /// The backup state to store: it may change.
    pub backed_up: bool,
    /// What the counter said. It is [`Counter::Flagged`] or [`Counter::Rose`] or [`Counter::Unused`] here;
    /// a refused one is no [`Verified`].
    pub judged: Counter,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_counter_is_used_only_when_it_is_used() {
        assert_eq!(Counter::judge(0, 0, true), Counter::Unused);
        assert_eq!(Counter::judge(0, 0, false), Counter::Unused);
        assert_eq!(Counter::judge(0, 1, false), Counter::Rose);
        assert_eq!(Counter::judge(5, 6, false), Counter::Rose);
        assert_eq!(Counter::judge(5, 5, false), Counter::Refused);
        assert_eq!(Counter::judge(5, 0, false), Counter::Refused);
        assert_eq!(Counter::judge(5, 3, true), Counter::Flagged);
    }

    #[test]
    fn options_use_webauthns_names() {
        let asking = Asking {
            challenge: Bytes::new(vec![1, 2]),
            rp_id: "app.uses.ink".into(),
            timeout: 300_000,
            user_verification: Requirement::Required,
            allow_credentials: Vec::new(),
        };
        let json = serde_json::to_value(&asking).unwrap();
        assert_eq!(json["rpId"], "app.uses.ink");
        assert_eq!(json["userVerification"], "required");
        assert_eq!(json["allowCredentials"], serde_json::json!([]));
        let creation = serde_json::to_value(Algorithm::ES256).unwrap();
        assert_eq!(creation, serde_json::json!({"type": "public-key", "alg": -7}));
    }
}
