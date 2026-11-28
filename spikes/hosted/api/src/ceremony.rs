//! A passkey ceremony through the server's `Webauthn` adapter, driven by a software authenticator
//! (a P-256 key, attestation `none`, ES256), as `crates/server/edge/api/tests/support` does.

use std::collections::BTreeMap;

use control::keys::Label;
use control::passkeys::{Asserted, Created, Passkeys, Person, Relying};
use control::{AccountKind, Id};
use edge_api::Webauthn;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde_cbor_2::Value;
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use sig::Base64;

/// What the run says, a line each.
#[derive(Default)]
pub struct Lines(Vec<String>);

impl Lines {
    fn say(&mut self, kind: &str, name: &str, detail: impl std::fmt::Display) {
        self.0.push(format!("{kind} {name} {detail}"));
    }

    /// The lines, as the body.
    pub fn text(&self) -> String {
        self.0.join("\n") + "\n"
    }
}

/// A passkey on a person's phone or a security key.
struct Authenticator {
    key: SigningKey,
    id: Vec<u8>,
    origin: String,
    rp_id: String,
}

impl Authenticator {
    fn new(origin: &str, rp_id: &str) -> Result<Self, String> {
        let key = SigningKey::from_bytes(&[7_u8; 32].into()).map_err(|error| format!("{error}"))?;
        Ok(Self { key, id: Sha256::digest([7_u8]).to_vec(), origin: origin.to_owned(), rp_id: rp_id.to_owned() })
    }

    fn head(&self, flags: u8, counter: u32) -> Vec<u8> {
        let mut data = Sha256::digest(self.rp_id.as_bytes()).to_vec();
        // present, user verified, plus what the caller adds
        data.push(0x01 | 0x04 | flags);
        data.extend_from_slice(&counter.to_be_bytes());
        data
    }

    fn cose(&self) -> Result<Vec<u8>, String> {
        let point = self.key.verifying_key().to_sec1_point(false);
        let x = point.x().ok_or("no x")?.to_vec();
        let y = point.y().ok_or("no y")?.to_vec();
        let map: BTreeMap<Value, Value> = BTreeMap::from([
            (Value::Integer(1), Value::Integer(2)),
            (Value::Integer(3), Value::Integer(-7)),
            (Value::Integer(-1), Value::Integer(1)),
            (Value::Integer(-2), Value::Bytes(x)),
            (Value::Integer(-3), Value::Bytes(y)),
        ]);
        serde_cbor_2::to_vec(&Value::Map(map)).map_err(|error| error.to_string())
    }

    fn client(&self, kind: &str, challenge: &Json, origin: &str) -> Vec<u8> {
        json!({"type": kind, "challenge": challenge, "origin": origin, "crossOrigin": false}).to_string().into_bytes()
    }

    fn create(&self, options: &Json) -> Result<Json, String> {
        let client = self.client("webauthn.create", &options["challenge"], &self.origin);
        let mut data = self.head(0x40, 0);
        data.extend_from_slice(&[0; 16]);
        data.extend_from_slice(&u16::try_from(self.id.len()).map_err(|error| error.to_string())?.to_be_bytes());
        data.extend_from_slice(&self.id);
        data.extend_from_slice(&self.cose()?);
        let object: BTreeMap<Value, Value> = BTreeMap::from([
            (Value::Text("fmt".into()), Value::Text("none".into())),
            (Value::Text("attStmt".into()), Value::Map(BTreeMap::new())),
            (Value::Text("authData".into()), Value::Bytes(data)),
        ]);
        let attestation = serde_cbor_2::to_vec(&Value::Map(object)).map_err(|error| error.to_string())?;
        Ok(json!({
            "id": Base64::encode(&self.id),
            "clientData": Base64::encode(&client),
            "attestation": Base64::encode(&attestation),
            "transports": ["internal"],
        }))
    }

    fn get(&self, options: &Json, counter: u32, origin: &str, flip: bool) -> Json {
        let client = self.client("webauthn.get", &options["challenge"], origin);
        let data = self.head(0, counter);
        let mut signed = data.clone();
        signed.extend_from_slice(&Sha256::digest(&client));
        let signature: Signature = self.key.sign(&signed);
        let mut der = signature.to_der().as_bytes().to_vec();
        if flip && let Some(last) = der.last_mut() {
            *last ^= 1;
        }
        json!({
            "id": Base64::encode(&self.id),
            "clientData": Base64::encode(&client),
            "authenticator": Base64::encode(&data),
            "signature": Base64::encode(&der),
            "handle": Base64::encode(&[7_u8; 16]),
        })
    }
}

/// The run.
pub struct Ceremony;

impl Ceremony {
    /// Registers a passkey, signs in with it, and tries four answers that must be refused.
    pub fn run() -> Lines {
        let mut lines = Lines::default();
        let started = worker::js_sys::Date::now();
        match Self::all(&mut lines) {
            Ok(()) => {}
            Err(error) => lines.say("fail", "ceremony", error),
        }
        lines.say("note", "elapsed", format!("{} ms for the whole ceremony, registration to refusals", worker::js_sys::Date::now() - started));
        lines
    }

    fn all(lines: &mut Lines) -> Result<(), String> {
        let relying = Relying::hosted();
        let origin = relying.origins().first().cloned().ok_or("no origin")?;
        let verifier = Webauthn::new(relying.clone()).map_err(|error| format!("{error:?}"))?;
        let phone = Authenticator::new(&origin, relying.id())?;
        let person = Person {
            account: Id::<AccountKind>::from_bytes([7; 16]),
            label: Label::parse("ada").map_err(|error| error.to_string())?,
        };
        let (creation, pending) = verifier.start_registration(&person, &[]).map_err(|error| format!("{error:?}"))?;
        let options = serde_json::to_value(&creation).map_err(|error| error.to_string())?;
        let created: Created = serde_json::from_value(phone.create(&options)?).map_err(|error| error.to_string())?;
        let stored = verifier.finish_registration(&pending, &created).map_err(|error| format!("{error:?}"))?;
        lines.say("ok", "registration", format!("credential of {} bytes, eligible={}", stored.credential.as_slice().len(), stored.eligible));

        let mut counter = 0_u32;
        for (name, origin_for, flip, accepted) in [
            ("login", origin.as_str(), false, true),
            ("tampered signature", origin.as_str(), true, false),
            ("wrong origin", "https://evil.example", false, false),
        ] {
            counter += 1;
            let (asking, state) = verifier.start_login().map_err(|error| format!("{error:?}"))?;
            let options = serde_json::to_value(&asking).map_err(|error| error.to_string())?;
            let reply: Asserted =
                serde_json::from_value(phone.get(&options, counter, origin_for, flip)).map_err(|error| error.to_string())?;
            let claim = verifier.who(&reply).map_err(|error| format!("{error:?}"))?;
            let verdict = verifier.finish_login(&state, &reply, &stored);
            match (verdict, accepted) {
                (Ok(done), true) => lines.say("ok", name, format!("counter {} judged {:?}, account {:?}", done.counter, done.judged, claim.account)),
                (Err(refusal), false) => lines.say("ok", name, format!("refused: {refusal:?}")),
                (Ok(_), false) => lines.say("fail", name, "was accepted"),
                (Err(refusal), true) => lines.say("fail", name, format!("was refused: {refusal:?}")),
            }
        }
        // an answer made for another challenge
        let (_, state) = verifier.start_login().map_err(|error| format!("{error:?}"))?;
        let (other, _) = verifier.start_login().map_err(|error| format!("{error:?}"))?;
        let options = serde_json::to_value(&other).map_err(|error| error.to_string())?;
        let reply: Asserted =
            serde_json::from_value(phone.get(&options, 9, &origin, false)).map_err(|error| error.to_string())?;
        match verifier.finish_login(&state, &reply, &stored) {
            Err(refusal) => lines.say("ok", "other challenge", format!("refused: {refusal:?}")),
            Ok(_) => lines.say("fail", "other challenge", "was accepted"),
        }
        Ok(())
    }
}
