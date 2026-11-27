//! A software authenticator: a P-256 key, a credential id and the bytes a browser would send, built
//! with this test's own signer (attestation `none`, ES256) and nothing from the verifier under
//! test. Every knob a refusal test needs is a field: the origin it claims, the relying party id it
//! signs for, the flags it sets and the counter it reports. The key comes from fixed bytes, so no
//! test draws randomness.

use std::collections::BTreeMap;

use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde_cbor_2::Value;
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use sig::Base64;

/// A passkey on a person's phone or a security key.
#[allow(unreachable_pub, reason = "a module of one test binary")]
pub struct Authenticator {
    key: SigningKey,
    /// The credential id the authenticator made.
    pub id: Vec<u8>,
    /// The origin the browser reports in the client data.
    pub origin: String,
    /// The relying party id the authenticator signs for.
    pub rp_id: String,
    /// The user verification flag.
    pub verified: bool,
    /// Backup eligibility: a synced passkey sets it.
    pub eligible: bool,
    /// Backup state.
    pub backed_up: bool,
    /// The counter reported by the next call.
    pub counter: u32,
}

#[allow(unreachable_pub, reason = "a module of one test binary")]
impl Authenticator {
    const PRESENT: u8 = 0x01;
    const VERIFIED: u8 = 0x04;
    const ELIGIBLE: u8 = 0x08;
    const BACKED_UP: u8 = 0x10;
    const ATTESTED: u8 = 0x40;

    /// A hardware key that verifies the person, keeps a counter and is not synced, from `seed`.
    pub fn hardware(seed: u8, origin: &str, rp_id: &str) -> Self {
        let key = SigningKey::from_bytes(&[seed.max(1); 32].into()).expect("a small nonzero scalar is a key");
        Self {
            key,
            id: Sha256::digest([seed]).to_vec(),
            origin: origin.to_owned(),
            rp_id: rp_id.to_owned(),
            verified: true,
            eligible: false,
            backed_up: false,
            counter: 0,
        }
    }

    /// A synced passkey: backup eligible and backed up, and it keeps no counter.
    pub fn synced(seed: u8, origin: &str, rp_id: &str) -> Self {
        Self { eligible: true, backed_up: true, ..Self::hardware(seed, origin, rp_id) }
    }

    fn flags(&self, extra: u8) -> u8 {
        Self::PRESENT
            | extra
            | if self.verified { Self::VERIFIED } else { 0 }
            | if self.eligible { Self::ELIGIBLE } else { 0 }
            | if self.backed_up { Self::BACKED_UP } else { 0 }
    }

    /// `rpIdHash || flags || counter`, the head of the authenticator data.
    fn head(&self, extra: u8) -> Vec<u8> {
        let mut data = Sha256::digest(self.rp_id.as_bytes()).to_vec();
        data.push(self.flags(extra));
        data.extend_from_slice(&self.counter.to_be_bytes());
        data
    }

    /// The public key as a COSE key: `{1: 2, 3: -7, -1: 1, -2: x, -3: y}`.
    fn cose(&self) -> Vec<u8> {
        let point = self.key.verifying_key().to_sec1_point(false);
        let (x, y) = (point.x().expect("uncompressed").to_vec(), point.y().expect("uncompressed").to_vec());
        let map: BTreeMap<Value, Value> = BTreeMap::from([
            (Value::Integer(1), Value::Integer(2)),
            (Value::Integer(3), Value::Integer(-7)),
            (Value::Integer(-1), Value::Integer(1)),
            (Value::Integer(-2), Value::Bytes(x)),
            (Value::Integer(-3), Value::Bytes(y)),
        ]);
        serde_cbor_2::to_vec(&Value::Map(map)).expect("a COSE key is CBOR")
    }

    fn client(&self, kind: &str, options: &Json) -> Vec<u8> {
        let challenge = options["challenge"].as_str().expect("options hold a challenge");
        json!({"type": kind, "challenge": challenge, "origin": self.origin, "crossOrigin": false})
            .to_string()
            .into_bytes()
    }

    /// The browser's answer to `navigator.credentials.create(options)`, as the body the script posts.
    pub fn create(&self, options: &Json) -> Json {
        let client = self.client("webauthn.create", options);
        let mut data = self.head(Self::ATTESTED);
        data.extend_from_slice(&[0; 16]);
        data.extend_from_slice(&u16::try_from(self.id.len()).expect("short id").to_be_bytes());
        data.extend_from_slice(&self.id);
        data.extend_from_slice(&self.cose());
        let object: BTreeMap<Value, Value> = BTreeMap::from([
            (Value::Text("fmt".into()), Value::Text("none".into())),
            (Value::Text("attStmt".into()), Value::Map(BTreeMap::new())),
            (Value::Text("authData".into()), Value::Bytes(data)),
        ]);
        let attestation = serde_cbor_2::to_vec(&Value::Map(object)).expect("an attestation object is CBOR");
        json!({
            "id": Base64::encode(&self.id),
            "clientData": Base64::encode(&client),
            "attestation": Base64::encode(&attestation),
            "transports": ["internal"],
        })
    }

    /// The browser's answer to `navigator.credentials.get(options)` for the account `handle`: the
    /// signature is DER ECDSA over `authenticatorData || SHA-256(clientDataJSON)`.
    pub fn get(&self, options: &Json, handle: &[u8]) -> Json {
        let client = self.client("webauthn.get", options);
        let data = self.head(0);
        let mut signed = data.clone();
        signed.extend_from_slice(&Sha256::digest(&client));
        let signature: Signature = self.key.sign(&signed);
        json!({
            "id": Base64::encode(&self.id),
            "clientData": Base64::encode(&client),
            "authenticator": Base64::encode(&data),
            "signature": Base64::encode(signature.to_der().as_bytes()),
            "handle": Base64::encode(handle),
        })
    }
}
