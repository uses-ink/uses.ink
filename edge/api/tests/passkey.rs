//! Passkeys over the API crate's JSON: the full ceremonies through the real handlers, driven by a
//! software authenticator, and each way a ceremony is refused. No browser is involved.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::duration_suboptimal_units,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_edge_api as edge_api;

#[path = "support/authenticator.rs"]
mod authenticator;

use std::sync::Arc;
use std::time::Duration;

use authenticator::Authenticator;
use blob_dir::Directory;
use clock::{Clock, Counter, Manual};
use control::keys::Keys;
use control::passkeys::{Registry, Relying};
use control::signin::{Drawn, Entry, Presented, Requested, Settings, SignIn, Signup};
use control::{AccountKind, Control, Domain, Id, Origin};
use db::{Db, Steps};
use db_sqlite::Sqlite;
use edge_api::{
    Answers, Api, Ceremonies, Closing, Door, Enrol, Jar, Limits, Method, NoOperators, Passing, Payload, Reply, Request,
    Routed, Signatures, Signing, Status, Webauthn,
};
use jiff::Timestamp;
use mail::{Address, Base};
use queue_local::{Local as Queue, Queued};
use secret_local::Local as Sealer;
use serde_json::{Value, json};

type Store = Arc<Sqlite>;
type Ports = (Store, Directory, Queue<Store>, Sealer);
type Gates = Ceremonies<Store, &'static Manual, Counter, Webauthn>;
type Front = Routed<
    Routed<
        Api<Ports, Signatures<Store, &'static Manual, NoOperators>, &'static Manual, Counter>,
        Door<Store, &'static Manual, Counter>,
    >,
    Gates,
>;

const ORIGIN: &str = "https://app.uses.ink";
const HOST: &str = "app.uses.ink";
const THIRTY_DAYS: u64 = 30 * 24 * 3600;

struct Fixture {
    front: Front,
    clock: &'static Manual,
    entry: SignIn<Store>,
    registry: Registry<Store>,
    keys: Keys<Store>,
    db: Store,
    draws: std::cell::Cell<u8>,
    _folder: tempfile::TempDir,
}

fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

impl Fixture {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let db: Store = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new()
            .with::<Domain>()
            .unwrap()
            .with::<Queued>()
            .unwrap()
            .with::<Entry>()
            .unwrap()
            .with::<control::keys::Entry>()
            .unwrap()
            .with::<control::passkeys::Entry>()
            .unwrap()
            .with::<control::recovery::Entry>()
            .unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (
            db.clone(),
            Directory::open(folder.path().join("blobs")).unwrap(),
            Queue::new(db.clone()),
            Sealer::ephemeral().unwrap(),
        );
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let clock: &'static Manual = Box::leak(Box::new(Manual::new(start(), 0)));
        let entry = SignIn::new(db.clone(), Settings { signup: Signup::Open, ..Settings::default() });
        let enrol = Enrol::new(db.clone(), control::keys::Settings::default(), Closing::default());
        let audience = sig::Audience::parse("api.example").unwrap();
        let config = Signing::new(audience, Id::from_bytes([9; 16]));
        let signatures = Signatures::new(enrol.keys().clone(), (clock, NoOperators), config);
        let keys = enrol.keys().clone();
        let api = Api::new(
            Control::new(ports, origin),
            (signatures, enrol.clone()),
            (clock, Counter::new()),
            Limits::default(),
        );
        let door_config = edge_api::door::Config {
            jar: jar(),
            base: "https://app.uses.ink/login".parse::<Base>().unwrap(),
            rates: edge_api::door::Rates::default(),
            outbox: edge_api::door::Outbox::new(4),
            reveals: false,
        };
        let door = Door::new(entry.clone(), (clock, Counter::new()), enrol, door_config);
        let registry = Registry::new(db.clone());
        Self {
            front: Routed::new(Routed::new(api, door), Self::gates_on(&db, &entry, clock, Relying::hosted())),
            clock,
            entry,
            registry,
            keys,
            db,
            draws: std::cell::Cell::new(0),
            _folder: folder,
        }
    }

    /// The passkey routes for `relying` over this database.
    fn gates_on(db: &Store, entry: &SignIn<Store>, clock: &'static Manual, relying: Relying) -> Gates {
        let config = Passing { session: jar(), ceremony: ceremony_jar(), relying: relying.clone() };
        Ceremonies::new(
            (entry.clone(), Registry::new(db.clone())),
            (clock, Counter::new()),
            Webauthn::new(relying).unwrap(),
            config,
        )
    }

    /// Signs a new person in by a link, and returns the account and the session's secret.
    fn person(&self, address: &str) -> (Id<AccountKind>, String) {
        let n = self.draws.get() + 1;
        self.draws.set(n);
        let address: Address = address.parse().unwrap();
        let requested =
            pollster::block_on(self.entry.request(&address, Drawn::new([[n; 16]; 5]), Clock::now(self.clock))).unwrap();
        let Requested::Issued(issued) = requested else { panic!("sign-up is open") };
        let entered = pollster::block_on(self.entry.redeem(
            Presented { proof: &issued.proof, address: None },
            Drawn::new([[n.wrapping_add(100); 16]; 5]),
            Clock::now(self.clock),
        ))
        .unwrap();
        (entered.account, entered.secret.reveal().to_owned())
    }

    fn send(&self, (method, path): (Method, &str), cookie: &str, body: &Value) -> Reply {
        self.send_with((method, path), Fetch { origin: Some(ORIGIN), site: None }, cookie, body)
    }

    fn send_with(&self, (method, path): (Method, &str), fetch: Fetch<'_>, cookie: &str, body: &Value) -> Reply {
        let request = Request {
            method,
            path: path.to_owned(),
            query: None,
            authorization: None,
            cookie: (!cookie.is_empty()).then(|| cookie.as_bytes().to_vec()),
            origin: fetch.origin.map(str::to_owned),
            site: fetch.site.map(str::to_owned),
            source: Some("10.0.0.1".parse().unwrap()),
            body: Payload::Whole(if body.is_null() { Vec::new() } else { body.to_string().into_bytes() }),
        };
        pollster::block_on(self.front.answer(&request))
    }

    /// Registers `authenticator` for the person with `session`; the reply of the last step.
    fn register(&self, session: &str, authenticator: &Authenticator) -> Reply {
        let jar = format!("__Host-ink-session={session}");
        let options = self.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": "phone"}));
        assert_eq!(options.status, Status::Ok, "{}", text(&options));
        let both = format!("{jar}; {}", pair(options.ceremony.as_ref().unwrap()));
        let credential = authenticator.create(&body(&options));
        self.send((Method::Post, "/v1/passkeys"), &both, &json!({"label": "phone", "credential": credential}))
    }

    /// Starts a sign-in: the options and the ceremony cookie.
    fn challenge(&self) -> (Value, String) {
        let reply = self.send((Method::Post, "/v1/passkeys/challenge"), "", &Value::Null);
        assert_eq!(reply.status, Status::Ok, "{}", text(&reply));
        (body(&reply), pair(reply.ceremony.as_ref().unwrap()))
    }

    /// A whole sign-in by `authenticator` for the account `handle`.
    fn sign_in(&self, authenticator: &Authenticator, handle: &[u8]) -> Reply {
        let (options, ceremony) = self.challenge();
        let credential = authenticator.get(&options, handle);
        self.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({"credential": credential}))
    }
}

#[derive(Clone, Copy)]
struct Fetch<'a> {
    origin: Option<&'a str>,
    site: Option<&'a str>,
}

fn jar() -> Jar {
    Jar::new(None, "/", true).unwrap()
}

fn ceremony_jar() -> Jar {
    Jar::new(Some("__Host-ink-ceremony".parse().unwrap()), "/", true).unwrap()
}

/// `name=value` of a `Set-Cookie`.
fn pair(cookie: &edge_api::Cookie) -> String {
    cookie.to_string().split(';').next().unwrap().to_owned()
}

fn body(reply: &Reply) -> Value {
    serde_json::from_slice(&reply.body).unwrap_or(Value::Null)
}

fn text(reply: &Reply) -> String {
    String::from_utf8_lossy(&reply.body).into_owned()
}

fn code(reply: &Reply) -> String {
    body(reply)["problem"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn a_passkey_registers_and_signs_in_and_the_session_is_a_cookie_only() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    let added = fixture.register(&secret, &phone);
    assert_eq!(added.status, Status::Created, "{}", text(&added));
    assert!(added.ceremony.as_ref().unwrap().to_string().contains("Max-Age=0"), "the spent cookie is cleared");
    let jar = format!("__Host-ink-session={secret}");
    let listed = fixture.send((Method::Get, "/v1/passkeys"), &jar, &Value::Null);
    let rows = body(&listed);
    assert_eq!(rows.as_array().unwrap().len(), 1, "{rows}");
    assert_eq!((rows[0]["label"].clone(), rows[0]["synced"].clone()), (json!("phone"), json!(true)));
    assert!(rows[0].get("key").is_none(), "no key material is listed");

    // sign in with no username: the credential names the account
    let signed = fixture.sign_in(&phone, account.as_bytes());
    assert_eq!(signed.status, Status::Ok, "{}", text(&signed));
    let shown = body(&signed);
    assert_eq!(shown["account"], json!(account.to_string()));
    assert!(shown.get("session").is_none(), "no secret is in the body: it is a cookie only");
    let cookie = signed.cookie.as_ref().unwrap().to_string();
    assert!(cookie.starts_with("__Host-ink-session="), "{cookie}");
    for attribute in ["Path=/", "HttpOnly", "SameSite=Lax", "Secure", &format!("Max-Age={THIRTY_DAYS}")] {
        assert!(cookie.contains(attribute), "{attribute} in {cookie}");
    }
    assert!(!cookie.contains("Domain"), "{cookie}");
    let value = cookie.split(';').next().unwrap().split_once('=').unwrap().1.to_owned();
    assert_eq!(value.len(), 64, "a 256-bit id, 64 hex digits");

    // cookies and signatures never meet: the cookie opens the dashboard routes and the API refuses it
    let session = format!("__Host-ink-session={value}");
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &session, &Value::Null).status, Status::Ok);
    let sites = fixture.send((Method::Get, "/v1/sites"), &session, &Value::Null);
    assert_eq!(sites.status, Status::BadRequest, "the API reads signatures and no cookie");
    let whoami = fixture.send((Method::Get, "/v1/whoami"), &session, &Value::Null);
    assert_eq!(whoami.status, Status::BadRequest, "a passkey session is no signature");
    // and a signature is no session here
    let signature = Request {
        method: Method::Get,
        path: "/v1/passkeys".to_owned(),
        query: None,
        authorization: Some(b"Ink-Sig-1 k=x".to_vec()),
        cookie: None,
        origin: None,
        site: None,
        source: None,
        body: Payload::Whole(Vec::new()),
    };
    assert_eq!(pollster::block_on(fixture.front.answer(&signature)).status, Status::Unauthorized);

    // the day it signed in is stored, and the events are the lifecycle
    let events: Vec<String> =
        pollster::block_on(fixture.keys.events(account, 10)).unwrap().into_iter().map(|event| event.name).collect();
    assert_eq!(events, ["passkey.signed-in", "passkey.added"]);
    let stored = pollster::block_on(fixture.registry.list(account)).unwrap();
    assert_eq!(stored[0].used, Some(jiff::civil::date(2027, 1, 15)));
}

#[test]
fn a_session_lasts_thirty_days_and_ends_after_seven_idle() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    let signed = fixture.sign_in(&phone, account.as_bytes());
    let cookie = pair(signed.cookie.as_ref().unwrap());
    let get = |fixture: &Fixture| fixture.send((Method::Get, "/v1/passkeys"), &cookie, &Value::Null).status;
    // used every six days, it lasts past a week and ends at the 30 day limit
    for _ in 0..4 {
        fixture.clock.advance(Duration::from_secs(6 * 24 * 3600));
        assert_eq!(get(&fixture), Status::Ok);
    }
    fixture.clock.advance(Duration::from_secs(6 * 24 * 3600 + 3600));
    assert_eq!(get(&fixture), Status::Unauthorized, "30 days is the absolute limit");
    // unused for a week it ends
    let signed = fixture.sign_in(&phone, account.as_bytes());
    let cookie = pair(signed.cookie.as_ref().unwrap());
    fixture.clock.advance(Duration::from_secs(7 * 24 * 3600 + 60));
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &cookie, &Value::Null).status, Status::Unauthorized);
}

#[test]
fn a_wrong_origin_is_refused_by_the_client_data_and_by_the_header() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    // registration from a page of another origin
    let evil = Authenticator::synced(1, "https://evil.example", HOST);
    let reply = fixture.register(&secret, &evil);
    assert_eq!((reply.status, code(&reply)), (Status::Unprocessable, "auth.passkey.origin".to_owned()));
    // a good registration, then a sign-in claiming another origin
    let mut phone = Authenticator::synced(2, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    phone.origin = "https://app.uses.ink.evil.example".to_owned();
    let reply = fixture.sign_in(&phone, account.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.origin".to_owned()));
    // the Origin header: a foreign one, none at all, and the browser's own `same-origin`
    let jar = format!("__Host-ink-session={secret}");
    let foreign = Fetch { origin: Some("https://evil.example"), site: None };
    let reply = fixture.send_with((Method::Post, "/v1/passkeys/options"), foreign, &jar, &json!({"label": "x"}));
    assert_eq!((reply.status, code(&reply)), (Status::Forbidden, "auth.origin.refused".to_owned()));
    let neither = Fetch { origin: None, site: None };
    assert_eq!(
        fixture.send_with((Method::Post, "/v1/passkeys/challenge"), neither, "", &Value::Null).status,
        Status::Forbidden
    );
    let cross = Fetch { origin: None, site: Some("cross-site") };
    assert_eq!(
        fixture.send_with((Method::Post, "/v1/passkeys/challenge"), cross, "", &Value::Null).status,
        Status::Forbidden
    );
    let same = Fetch { origin: None, site: Some("same-origin") };
    assert_eq!(fixture.send_with((Method::Post, "/v1/passkeys/challenge"), same, "", &Value::Null).status, Status::Ok);
    // a read needs no origin
    let read = Fetch { origin: None, site: None };
    assert_eq!(fixture.send_with((Method::Get, "/v1/passkeys"), read, &jar, &Value::Null).status, Status::Ok);
}

#[test]
fn a_wrong_relying_party_id_is_refused() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let other = Authenticator::synced(1, ORIGIN, "uses.ink");
    let reply = fixture.register(&secret, &other);
    assert_eq!((reply.status, code(&reply)), (Status::Unprocessable, "auth.passkey.relying-party".to_owned()));
    let mut phone = Authenticator::synced(2, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    phone.rp_id = "uses.ink".to_owned();
    let reply = fixture.sign_in(&phone, account.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.relying-party".to_owned()));
}

#[test]
fn a_challenge_is_used_once_and_expires() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    // replay: the same ceremony and the same assertion twice
    let (options, ceremony) = fixture.challenge();
    let credential = phone.get(&options, account.as_bytes());
    let post = || fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({"credential": credential}));
    assert_eq!(post().status, Status::Ok);
    let again = post();
    assert_eq!((again.status, code(&again)), (Status::Conflict, "auth.passkey.challenge".to_owned()));
    // an assertion for another ceremony's challenge: refused, and the ceremony is spent
    let (_, ceremony_a) = fixture.challenge();
    let (options_b, _) = fixture.challenge();
    let credential = phone.get(&options_b, account.as_bytes());
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony_a, &json!({"credential": credential}));
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.challenge".to_owned()));
    let retry = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony_a, &json!({"credential": credential}));
    assert_eq!(code(&retry), "auth.passkey.challenge");
    // no cookie: another browser cannot complete it
    let (options, _) = fixture.challenge();
    let credential = phone.get(&options, account.as_bytes());
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), "", &json!({"credential": credential}));
    assert_eq!((reply.status, code(&reply)), (Status::Conflict, "auth.passkey.challenge".to_owned()));
    // five minutes
    let (options, ceremony) = fixture.challenge();
    let credential = phone.get(&options, account.as_bytes());
    fixture.clock.advance(Duration::from_secs(301));
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({"credential": credential}));
    assert_eq!((reply.status, code(&reply)), (Status::Conflict, "auth.passkey.challenge".to_owned()));
    // and a registration ceremony does not complete a sign-in
    let fresh = pair(fixture.sign_in(&phone, account.as_bytes()).cookie.as_ref().unwrap());
    let jar = fresh;
    let options = fixture.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": "second"}));
    let ceremony = pair(options.ceremony.as_ref().unwrap());
    let credential = phone.get(&body(&options), account.as_bytes());
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({"credential": credential}));
    assert_eq!(code(&reply), "auth.passkey.challenge");
}

#[test]
fn a_counter_that_runs_backwards_refuses_a_hardware_key_and_flags_a_synced_passkey() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let mut key = Authenticator::hardware(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &key).status, Status::Created);
    key.counter = 5;
    assert_eq!(fixture.sign_in(&key, account.as_bytes()).status, Status::Ok);
    key.counter = 6;
    assert_eq!(fixture.sign_in(&key, account.as_bytes()).status, Status::Ok);
    key.counter = 6;
    let reply = fixture.sign_in(&key, account.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.counter".to_owned()));
    key.counter = 2;
    assert_eq!(code(&fixture.sign_in(&key, account.as_bytes())), "auth.passkey.counter");
    key.counter = 7;
    assert_eq!(fixture.sign_in(&key, account.as_bytes()).status, Status::Ok, "a rising counter signs in again");
    let events: Vec<String> =
        pollster::block_on(fixture.keys.events(account, 20)).unwrap().into_iter().map(|event| event.name).collect();
    assert_eq!(events.iter().filter(|name| *name == "passkey.flagged").count(), 2, "{events:?}");

    // a synced passkey reports zero forever, and a value that goes back is flagged, not refused
    let (account, secret) = fixture.person("you@example.org");
    let mut synced = Authenticator::synced(2, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &synced).status, Status::Created);
    assert_eq!(fixture.sign_in(&synced, account.as_bytes()).status, Status::Ok);
    synced.counter = 9;
    assert_eq!(fixture.sign_in(&synced, account.as_bytes()).status, Status::Ok);
    synced.counter = 3;
    assert_eq!(fixture.sign_in(&synced, account.as_bytes()).status, Status::Ok);
    let events = pollster::block_on(fixture.keys.events(account, 20)).unwrap();
    assert!(events.iter().any(|event| event.name == "passkey.flagged" && event.body.contains("\"refused\":false")));
}

#[test]
fn an_unknown_credential_and_another_accounts_are_one_answer() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    let stranger = Authenticator::synced(7, ORIGIN, HOST);
    let reply = fixture.sign_in(&stranger, account.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.refused".to_owned()));
    // the right passkey naming another account's handle
    let (other, _) = fixture.person("you@example.org");
    let reply = fixture.sign_in(&phone, other.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.refused".to_owned()));
    // a handle that is no account id
    let reply = fixture.sign_in(&phone, b"short");
    assert_eq!(code(&reply), "auth.passkey.refused");
    // a bad signature
    let (options, ceremony) = fixture.challenge();
    let mut credential = phone.get(&options, account.as_bytes());
    let signature = Authenticator::synced(9, ORIGIN, HOST).get(&options, account.as_bytes())["signature"].clone();
    credential["signature"] = signature;
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({"credential": credential}));
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.refused".to_owned()));
}

#[test]
fn user_verification_is_required_at_registration_and_at_every_sign_in() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let mut unverified = Authenticator::synced(1, ORIGIN, HOST);
    unverified.verified = false;
    let reply = fixture.register(&secret, &unverified);
    assert_eq!((reply.status, code(&reply)), (Status::Unprocessable, "auth.passkey.unverified".to_owned()));
    let mut phone = Authenticator::synced(2, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    phone.verified = false;
    let reply = fixture.sign_in(&phone, account.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.unverified".to_owned()));
}

#[test]
fn backup_eligibility_may_not_change_after_registration() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let mut key = Authenticator::hardware(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &key).status, Status::Created);
    key.eligible = true;
    let reply = fixture.sign_in(&key, account.as_bytes());
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.passkey.backup".to_owned()));
    let (account, secret) = fixture.person("you@example.org");
    let mut phone = Authenticator::synced(2, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    phone.eligible = false;
    assert_eq!(code(&fixture.sign_in(&phone, account.as_bytes())), "auth.passkey.backup");
    // the backup state may change, and is stored
    phone.eligible = true;
    phone.backed_up = false;
    assert_eq!(fixture.sign_in(&phone, account.as_bytes()).status, Status::Ok);
    let stored = pollster::block_on(fixture.registry.list(account)).unwrap();
    assert!(!stored[0].stored.backed_up);
}

#[test]
fn one_authenticator_makes_one_passkey() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    // the options name the passkey so the authenticator skips it, and a reply for it anyway is refused
    let jar = pair(fixture.sign_in(&phone, account.as_bytes()).cookie.as_ref().unwrap());
    let options = fixture.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": "again"}));
    assert_eq!(body(&options)["excludeCredentials"].as_array().unwrap().len(), 1);
    let both = format!("{jar}; {}", pair(options.ceremony.as_ref().unwrap()));
    let credential = phone.create(&body(&options));
    let reply =
        fixture.send((Method::Post, "/v1/passkeys"), &both, &json!({"label": "again", "credential": credential}));
    assert_eq!(reply.status, Status::Unprocessable, "{}", text(&reply));
    assert_eq!(code(&reply), "auth.passkey.refused", "{}", text(&reply));
}

#[test]
fn the_options_are_what_the_design_says() {
    let fixture = Fixture::new();
    let (account, secret) = fixture.person("me@example.org");
    let jar = format!("__Host-ink-session={secret}");
    let reply = fixture.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": "phone"}));
    let options = body(&reply);
    assert_eq!(options["rp"]["id"], json!(HOST));
    assert_eq!(options["authenticatorSelection"], json!({"residentKey": "required", "userVerification": "required"}));
    assert_eq!(options["attestation"], json!("none"));
    assert_eq!(options["pubKeyCredParams"], json!([{"type": "public-key", "alg": -7}]));
    assert_eq!(options["user"]["id"], json!(sig::Base64::encode(account.as_bytes())));
    assert_eq!(sig::Base64::decode_vec(options["challenge"].as_str().unwrap()).unwrap().len(), 32);
    let cookie = reply.ceremony.as_ref().unwrap().to_string();
    for attribute in ["__Host-ink-ceremony=", "Secure", "HttpOnly", "SameSite=Lax", "Max-Age=300"] {
        assert!(cookie.contains(attribute), "{attribute} in {cookie}");
    }
    // the challenge never repeats
    let again = body(&fixture.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": "phone"})));
    assert_ne!(options["challenge"], again["challenge"]);
    let (login, _) = fixture.challenge();
    assert_eq!(login["rpId"], json!(HOST));
    assert_eq!(login["userVerification"], json!("required"));
    assert_eq!(login["allowCredentials"], json!([]));
    // a label that is no label, and a body that is no JSON object
    let bad = fixture.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": ""}));
    assert_eq!(code(&bad), "auth.passkey.label");
    assert_eq!(
        fixture.send((Method::Post, "/v1/passkeys/options"), &jar, &json!({"label": "x", "extra": 1})).status,
        Status::BadRequest
    );
    // registering needs a session
    assert_eq!(
        fixture.send((Method::Post, "/v1/passkeys/options"), "", &json!({"label": "x"})).status,
        Status::Unauthorized
    );
}

#[test]
fn removing_a_passkey_ends_the_sessions_it_made() {
    let fixture = Fixture::new();
    let (account, link_session) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&link_session, &phone).status, Status::Created);
    let by_phone = pair(fixture.sign_in(&phone, account.as_bytes()).cookie.as_ref().unwrap());
    let key = Authenticator::hardware(2, ORIGIN, HOST);
    // a second passkey is added by a session a passkey just made, not by the link's
    assert_eq!(fixture.register(by_phone.split_once('=').unwrap().1, &key).status, Status::Created);
    let by_key = pair(fixture.sign_in(&key, account.as_bytes()).cookie.as_ref().unwrap());
    let link = format!("__Host-ink-session={link_session}");
    let listed = body(&fixture.send((Method::Get, "/v1/passkeys"), &link, &Value::Null));
    let phone_id = listed.as_array().unwrap().iter().find(|row| row["synced"] == json!(true)).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let removed = fixture.send((Method::Delete, &format!("/v1/passkeys/{phone_id}")), &by_key, &Value::Null);
    assert_eq!(removed.status, Status::Ok, "{}", text(&removed));
    // the phone's session is gone, the key's and the link's are not
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &by_phone, &Value::Null).status, Status::Unauthorized);
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &by_key, &Value::Null).status, Status::Ok);
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &link, &Value::Null).status, Status::Ok);
    // the removed passkey signs in no more, and removing it again finds nothing
    assert_eq!(code(&fixture.sign_in(&phone, account.as_bytes())), "auth.passkey.refused");
    let again = fixture.send((Method::Delete, &format!("/v1/passkeys/{phone_id}")), &by_key, &Value::Null);
    assert_eq!((again.status, code(&again)), (Status::NotFound, "auth.passkey.missing".to_owned()));
    // another account cannot remove it either
    let (_, other) = fixture.person("you@example.org");
    let other = format!("__Host-ink-session={other}");
    let id =
        body(&fixture.send((Method::Get, "/v1/passkeys"), &link, &Value::Null))[0]["id"].as_str().unwrap().to_owned();
    let theirs = fixture.send((Method::Delete, &format!("/v1/passkeys/{id}")), &other, &Value::Null);
    assert_eq!(theirs.status, Status::NotFound);
    assert_eq!(
        fixture.send((Method::Delete, "/v1/passkeys/not-an-id"), &by_key, &Value::Null).status,
        Status::NotFound
    );
    let events: Vec<String> =
        pollster::block_on(fixture.keys.events(account, 20)).unwrap().into_iter().map(|event| event.name).collect();
    assert!(events.contains(&"passkey.removed".to_owned()), "{events:?}");
}

#[test]
fn a_changed_relying_party_id_is_an_error_and_not_a_silent_failure() {
    let fixture = Fixture::new();
    let (_, secret) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&secret, &phone).status, Status::Created);
    let moved = Relying::new("ink.example.com", "ink", &["https://ink.example.com"]).unwrap();
    let gates = Fixture::gates_on(&fixture.db, &fixture.entry, fixture.clock, moved);
    let request = Request {
        method: Method::Post,
        path: "/v1/passkeys/challenge".to_owned(),
        query: None,
        authorization: None,
        cookie: None,
        origin: Some("https://ink.example.com".to_owned()),
        site: None,
        source: None,
        body: Payload::Whole(Vec::new()),
    };
    let reply = pollster::block_on(gates.respond(&request));
    assert_eq!((reply.status, code(&reply)), (Status::Conflict, "auth.relying-party.changed".to_owned()));
    let fields = &body(&reply)["fields"];
    assert_eq!(
        (fields["was"].clone(), fields["now"].clone(), fields["passkeys"].clone()),
        (json!(HOST), json!("ink.example.com"), json!("1"))
    );
}

#[test]
fn bodies_and_routes_that_are_wrong_say_so() {
    let fixture = Fixture::new();
    let reply = fixture.send((Method::Get, "/v1/passkeys/challenge"), "", &Value::Null);
    assert_eq!(reply.status, Status::MethodNotAllowed);
    assert_eq!(reply.allow, [Method::Post]);
    let (_, ceremony) = fixture.challenge();
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({"credential": {"id": "x"}}));
    assert_eq!(reply.status, Status::BadRequest);
    let reply = fixture.send((Method::Post, "/v1/passkeys/session"), &ceremony, &json!({}));
    assert_eq!(reply.status, Status::BadRequest);
}

#[test]
fn starting_a_sign_in_is_limited_per_source_and_freed_by_the_clock() {
    let fixture = Fixture::new();
    for _ in 0..60 {
        fixture.challenge();
    }
    let over = fixture.send((Method::Post, "/v1/passkeys/challenge"), "", &Value::Null);
    assert_eq!((over.status, code(&over)), (Status::TooMany, "auth.rate.limited".to_owned()));
    assert!(over.retry.is_some_and(|retry| retry.as_secs() > 0), "the reply says when to try again");
    assert!(over.ceremony.is_none(), "a limited request starts nothing");
    fixture.clock.advance(Duration::from_secs(3601));
    fixture.challenge();
}

#[test]
fn adding_or_removing_a_passkey_needs_a_passkey_sign_in_within_the_window() {
    let fixture = Fixture::new();
    let (account, link) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    // no passkey yet: nothing to prove it with, so the link's session adds the first
    assert_eq!(fixture.register(&link, &phone).status, Status::Created);
    let link_jar = format!("__Host-ink-session={link}");
    let laptop = Authenticator::hardware(2, ORIGIN, HOST);
    let stale = |reply: &Reply| (reply.status, code(reply));
    let refused = (Status::Forbidden, "auth.step-up.required".to_owned());
    // now the link's session is no step-up, at the first step and at the last
    let options = fixture.send((Method::Post, "/v1/passkeys/options"), &link_jar, &json!({"label": "x"}));
    assert_eq!(stale(&options), refused);
    assert!(options.ceremony.is_none(), "nothing was started");
    let id = body(&fixture.send((Method::Get, "/v1/passkeys"), &link_jar, &Value::Null))[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let delete = |jar: &str| fixture.send((Method::Delete, &format!("/v1/passkeys/{id}")), jar, &Value::Null);
    assert_eq!(stale(&delete(&link_jar)), refused);
    // a passkey sign-in is the step-up; it replaces the session the browser held
    let (options, ceremony) = fixture.challenge();
    let credential = phone.get(&options, account.as_bytes());
    let both = format!("{link_jar}; {ceremony}");
    let signed = fixture.send((Method::Post, "/v1/passkeys/session"), &both, &json!({"credential": credential}));
    assert_eq!(signed.status, Status::Ok, "{}", text(&signed));
    let fresh = pair(signed.cookie.as_ref().unwrap());
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &link_jar, &Value::Null).status, Status::Unauthorized);
    let secret = fresh.split_once('=').unwrap().1.to_owned();
    assert_eq!(fixture.register(&secret, &laptop).status, Status::Created);
    // the window ends five minutes after the passkey made the session, and then every step is refused
    fixture.clock.advance(Duration::from_secs(299));
    let still = fixture.send((Method::Post, "/v1/passkeys/options"), &fresh, &json!({"label": "x"}));
    assert_eq!(still.status, Status::Ok);
    let started = format!("{fresh}; {}", pair(still.ceremony.as_ref().unwrap()));
    let credential = laptop.create(&body(&still));
    fixture.clock.advance(Duration::from_secs(2));
    let late = fixture.send((Method::Post, "/v1/passkeys/options"), &fresh, &json!({"label": "x"}));
    assert_eq!(stale(&late), refused);
    // a ceremony started in time is not finished late, and the answer is the step-up, not a dead ceremony
    let finish = json!({"label": "x", "credential": credential});
    assert_eq!(stale(&fixture.send((Method::Post, "/v1/passkeys"), &started, &finish)), refused);
    assert_eq!(stale(&delete(&fresh)), refused);
    // reading needs no step-up, and signing in again makes the session fresh
    assert_eq!(fixture.send((Method::Get, "/v1/passkeys"), &fresh, &Value::Null).status, Status::Ok);
    let again = pair(fixture.sign_in(&phone, account.as_bytes()).cookie.as_ref().unwrap());
    assert_eq!(delete(&again).status, Status::Ok);
}

#[test]
fn approving_a_key_needs_the_same_step_up() {
    let fixture = Fixture::new();
    let (account, link) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    let link_jar = format!("__Host-ink-session={link}");
    let device = sig::Signer::from_seed(&sig::Seed::new([3; 32]));
    let ask = json!({ "key": device.public().to_string(), "label": "laptop", "purpose": "device" });
    let fingerprint = json!({ "fingerprint": device.id().fingerprint().to_string() });
    let spare = sig::Signer::from_seed(&sig::Seed::new([4; 32]));
    let spare_ask = json!({ "key": spare.public().to_string(), "label": "spare", "purpose": "device" });
    let spare_fingerprint = json!({ "fingerprint": spare.id().fingerprint().to_string() });
    let enrol = |fixture: &Fixture, ask: &Value| {
        let asked = fixture.send((Method::Post, "/v1/enrol"), "", ask);
        assert_eq!(asked.status, Status::Created, "{}", text(&asked));
        format!("/v1/approve/{}", body(&asked)["user_code"].as_str().unwrap())
    };
    // with no passkey on the account the session is as strong as it gets, and approves
    let first = enrol(&fixture, &ask);
    assert_eq!(fixture.send((Method::Post, &first), &link_jar, &fingerprint).status, Status::Ok);
    // with a passkey, a session an email link made does not
    assert_eq!(fixture.register(&link, &phone).status, Status::Created);
    let second = enrol(&fixture, &spare_ask);
    let refused = fixture.send((Method::Post, &second), &link_jar, &spare_fingerprint);
    assert_eq!((refused.status, code(&refused)), (Status::Forbidden, "auth.step-up.required".to_owned()));
    assert_eq!(fixture.send((Method::Get, &second), &link_jar, &Value::Null).status, Status::Ok, "looking is free");
    // a passkey session does, until its window ends
    let fresh = pair(fixture.sign_in(&phone, account.as_bytes()).cookie.as_ref().unwrap());
    fixture.clock.advance(Duration::from_secs(301));
    let late = fixture.send((Method::Post, &second), &fresh, &spare_fingerprint);
    assert_eq!(code(&late), "auth.step-up.required");
    let fresh = pair(fixture.sign_in(&phone, account.as_bytes()).cookie.as_ref().unwrap());
    assert_eq!(fixture.send((Method::Post, &second), &fresh, &spare_fingerprint).status, Status::Ok);
}

/// Signs in with `authenticator` and returns the session cookie as `name=value`.
fn cookie_of(reply: &Reply) -> String {
    pair(reply.cookie.as_ref().unwrap())
}

#[test]
fn recovery_codes_are_shown_once_kept_as_digests_and_a_new_set_voids_the_old() {
    let fixture = Fixture::new();
    let (account, link) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    let link_jar = format!("__Host-ink-session={link}");
    // with no passkey the link's session may mint, and the first set is ten codes
    let first = fixture.send((Method::Post, "/v1/recovery/codes"), &link_jar, &Value::Null);
    assert_eq!(first.status, Status::Created, "{}", text(&first));
    let first: Vec<String> =
        body(&first)["codes"].as_array().unwrap().iter().map(|code| code.as_str().unwrap().to_owned()).collect();
    assert_eq!(first.len(), 10);
    assert!(first.iter().all(|code| code.len() == 33 && code.split('-').count() == 7), "{first:?}");
    assert_eq!(first.iter().collect::<std::collections::BTreeSet<_>>().len(), 10, "ten different codes");
    // only digests are at rest: the table has 32-byte digests and no code text, and so does the log
    let rows = pollster::block_on(
        fixture.db.rows(
            sea_query::Query::select()
                .columns([control::table::RecoveryCode::Digest, control::table::RecoveryCode::Id])
                .from(control::table::RecoveryCode::Table)
                .to_owned()
                .into(),
        ),
    )
    .unwrap();
    assert_eq!(rows.len(), 10);
    let events = pollster::block_on(fixture.keys.events(account, 50)).unwrap();
    let logged = format!("{events:?}");
    for code in &first {
        assert!(!logged.contains(&code.replace('-', "")) && !logged.contains(code), "a code is in the log");
    }
    // once a passkey exists the link's session no longer mints; a passkey sign-in does
    assert_eq!(fixture.register(&link, &phone).status, Status::Created);
    let refused = fixture.send((Method::Post, "/v1/recovery/codes"), &link_jar, &Value::Null);
    assert_eq!((refused.status, code(&refused)), (Status::Forbidden, "auth.step-up.required".to_owned()));
    let jar = cookie_of(&fixture.sign_in(&phone, account.as_bytes()));
    assert_eq!(body(&fixture.send((Method::Get, "/v1/recovery/codes"), &jar, &Value::Null))["remaining"], json!(10));
    let second = fixture.send((Method::Post, "/v1/recovery/codes"), &jar, &Value::Null);
    assert_eq!(second.status, Status::Created, "{}", text(&second));
    let second: Vec<String> =
        body(&second)["codes"].as_array().unwrap().iter().map(|code| code.as_str().unwrap().to_owned()).collect();
    assert!(first.iter().all(|old| !second.contains(old)));
    // the old set is void, the new one is not
    let old = fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": first[0], "label": "new"}));
    assert_eq!((old.status, code(&old)), (Status::Unauthorized, "auth.recovery.refused".to_owned()));
    assert_eq!(fixture.send((Method::Get, "/v1/recovery/codes"), "", &Value::Null).status, Status::Unauthorized);
    let events: Vec<String> =
        pollster::block_on(fixture.keys.events(account, 50)).unwrap().into_iter().map(|event| event.name).collect();
    assert_eq!(events.iter().filter(|name| *name == "recovery.minted").count(), 2, "{events:?}");
}

#[test]
fn a_recovery_code_is_spent_once_to_register_a_passkey_and_a_wrong_one_is_limited() {
    let fixture = Fixture::new();
    let (account, link) = fixture.person("me@example.org");
    let phone = Authenticator::synced(1, ORIGIN, HOST);
    assert_eq!(fixture.register(&link, &phone).status, Status::Created);
    let jar = cookie_of(&fixture.sign_in(&phone, account.as_bytes()));
    let minted = fixture.send((Method::Post, "/v1/recovery/codes"), &jar, &Value::Null);
    let codes: Vec<String> =
        body(&minted)["codes"].as_array().unwrap().iter().map(|code| code.as_str().unwrap().to_owned()).collect();
    // the phone is lost: with no session at all, a code starts a registration and clears any cookie
    let stale = format!("__Host-ink-session={link}");
    let started =
        fixture.send((Method::Post, "/v1/recovery"), &stale, &json!({"code": codes[0], "label": "new phone"}));
    assert_eq!(started.status, Status::Ok, "{}", text(&started));
    assert!(started.cookie.as_ref().unwrap().to_string().contains("Max-Age=0"), "a stale session cookie is dropped");
    let ceremony = pair(started.ceremony.as_ref().unwrap());
    let spare = Authenticator::hardware(5, ORIGIN, HOST);
    let credential = spare.create(&body(&started));
    let added = fixture.send(
        (Method::Post, "/v1/passkeys"),
        &ceremony,
        &json!({"label": "new phone", "credential": credential}),
    );
    assert_eq!(added.status, Status::Created, "{}", text(&added));
    // and the new passkey signs in
    assert_eq!(fixture.sign_in(&spare, account.as_bytes()).status, Status::Ok);
    // a code is spent: the same one again is the answer of a code that never was
    let again = fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": codes[0], "label": "x"}));
    assert_eq!((again.status, code(&again)), (Status::Unauthorized, "auth.recovery.refused".to_owned()));
    assert_eq!(body(&fixture.send((Method::Get, "/v1/recovery/codes"), &jar, &Value::Null))["remaining"], json!(9));
    // the code's case, dashes and look-alike letters do not matter
    let loose = codes[1].to_ascii_lowercase().replace('-', "");
    assert_eq!(
        fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": loose, "label": "x"})).status,
        Status::Ok
    );
    // a typo is caught before it is counted: any number of them, and then the real code still works
    for _ in 0..20 {
        let typo = fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": "ABCD-EFGH", "label": "x"}));
        assert_eq!((typo.status, code(&typo)), (Status::Unprocessable, "auth.recovery.refused".to_owned()));
    }
    // the cross-origin post of a code does nothing
    let cross = fixture.send_with(
        (Method::Post, "/v1/recovery"),
        Fetch { origin: Some("https://evil.example"), site: None },
        "",
        &json!({"code": codes[2], "label": "x"}),
    );
    assert_eq!((cross.status, code(&cross)), (Status::Forbidden, "auth.origin.refused".to_owned()));
    // ten tries a source an hour; a well-formed code that is no code of ours counts
    let never = control::recovery::Code::from_bytes([9; 16]).reveal();
    for _ in 0..7 {
        let miss = fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": never, "label": "x"}));
        assert_eq!(miss.status, Status::Unauthorized);
    }
    let limited = fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": codes[2], "label": "x"}));
    assert_eq!((limited.status, code(&limited)), (Status::TooMany, "auth.rate.limited".to_owned()));
    assert!(limited.retry.is_some());
    fixture.clock.advance(Duration::from_secs(3601));
    assert_eq!(
        fixture.send((Method::Post, "/v1/recovery"), "", &json!({"code": codes[2], "label": "x"})).status,
        Status::Ok
    );
    let events: Vec<String> =
        pollster::block_on(fixture.keys.events(account, 50)).unwrap().into_iter().map(|event| event.name).collect();
    assert_eq!(events.iter().filter(|name| *name == "recovery.used").count(), 3, "{events:?}");
}
