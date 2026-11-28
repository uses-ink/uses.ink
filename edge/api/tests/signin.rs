//! Signing in over the API: one reply for every address, single-use links, the session as a cookie
//! and revocation, rate limits, and no secret in anything that prints.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_edge_api as edge_api;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use blob_dir::Directory;
use clock::{Counter, Manual};
use control::signin::{Entry, Settings, SignIn, Signup};
use control::{Control, Domain, Origin};
use db::Steps;
use db_sqlite::Sqlite;
use edge_api::door::{Config, Courier, Note, Notes, Outbox, Rates};
use edge_api::{
    Answers, Api, Closing, Door, Enrol, Jar, Limits, Method, NoOperators, Payload, Reply, Request, Routed, Signatures,
    Signing, Status,
};
use jiff::Timestamp;
use mail::{Base, Capture};
use queue_local::{Local as Queue, Queued};
use secret_local::Local as Sealer;
use serde_json::{Value, json};

type Store = Arc<Sqlite>;
type Ports = (Store, Directory, Queue<Store>, Sealer);
type Front = Routed<
    Api<Ports, Signatures<Store, &'static Manual, NoOperators>, &'static Manual, Counter>,
    Door<Store, &'static Manual, Counter>,
>;

#[derive(Default)]
struct Heard(Mutex<Vec<String>>);

impl Notes for Heard {
    fn note(&self, note: &Note) {
        self.0.lock().unwrap().push(format!("{note:?}"));
    }
}

struct Fixture {
    front: Front,
    clock: &'static Manual,
    mail: &'static Capture,
    courier: Courier<&'static Capture, &'static Manual>,
    entry: SignIn<Store>,
    outbox: Outbox,
    _folder: tempfile::TempDir,
}

struct Wanted {
    settings: Settings,
    rates: Rates,
    secure: bool,
    reveals: bool,
    capacity: usize,
    dashboard: Option<&'static str>,
}

impl Default for Wanted {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            rates: Rates { tracked: 1000, ..Rates::default() },
            secure: false,
            reveals: false,
            capacity: 16,
            dashboard: None,
        }
    }
}

const BASE: &str = "https://ink.example/login";

fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

impl Fixture {
    #[allow(clippy::needless_pass_by_value, reason = "a test names what it wants and hands it over")]
    fn new(wanted: Wanted) -> Self {
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
        let mail: &'static Capture = Box::leak(Box::new(Capture::new()));
        let outbox = Outbox::new(wanted.capacity);
        let entry = SignIn::new(db.clone(), wanted.settings);
        let enrol = Enrol::new(db.clone(), control::keys::Settings::default(), Closing::default());
        let audience = sig::Audience::parse("api.example").unwrap();
        let config = Signing::new(audience, control::Id::from_bytes([9; 16]));
        let signatures = Signatures::new(enrol.keys().clone(), (clock, NoOperators), config);
        let api = Api::new(
            Control::new(ports, origin),
            (signatures, enrol.clone()),
            (clock, Counter::new()),
            Limits::default(),
        );
        let config = Config {
            jar: Jar::new(None, "/", wanted.secure).unwrap(),
            base: BASE.parse::<Base>().unwrap(),
            rates: wanted.rates,
            outbox: outbox.clone(),
            reveals: wanted.reveals,
        };
        let door = Door::new(entry.clone(), (clock, Counter::new()), enrol, config);
        let door = match wanted.dashboard {
            Some(origin) => door.with_origin(origin),
            None => door,
        };
        let courier = Courier::new(outbox.clone(), mail, clock, Duration::from_secs(1));
        Self { front: Routed::new(api, door), clock, mail, courier, entry, outbox, _folder: folder }
    }

    fn send(&self, (method, path): (Method, &str), held: Held<'_>, body: &[u8]) -> Reply {
        let request = Request {
            method,
            path: path.to_owned(),
            query: None,
            authorization: held.signed.map(|header| header.as_bytes().to_vec()),
            cookie: held.cookie.map(|cookie| cookie.as_bytes().to_vec()),
            origin: held.origin.map(str::to_owned),
            site: held.site.map(str::to_owned),
            source: Some(held.source.parse().unwrap()),
            body: Payload::Whole(body.to_vec()),
        };
        pollster::block_on(self.front.answer(&request))
    }

    fn ask(&self, address: &str, source: &str) -> Reply {
        let body = json!({ "address": address }).to_string();
        self.send((Method::Post, "/v1/login"), Held { source, ..Held::default() }, body.as_bytes())
    }

    /// The token in the last message the courier sent.
    fn token(&self) -> String {
        pollster::block_on(self.courier.step(&Heard::default()));
        let sent = self.mail.sent();
        let body = sent.last().unwrap().template.to_string();
        let link = body.split_whitespace().find(|word| word.starts_with(BASE)).unwrap();
        link.rsplit('/').next().unwrap().to_owned()
    }

    fn redeem(&self, token: &str) -> Reply {
        self.send((Method::Post, &format!("/v1/login/{token}")), Held::default(), b"")
    }

    /// Signs `address` in and returns the session's secret.
    fn sign_in(&self, address: &str) -> String {
        assert_eq!(self.ask(address, "10.0.0.1").status, Status::Accepted);
        let reply = self.redeem(&self.token());
        assert_eq!(reply.status, Status::Ok, "{}", String::from_utf8_lossy(&reply.body));
        json(&reply)["session"].as_str().unwrap().to_owned()
    }
}

#[derive(Clone, Copy)]
struct Held<'a> {
    signed: Option<&'a str>,
    cookie: Option<&'a str>,
    source: &'a str,
    origin: Option<&'a str>,
    site: Option<&'a str>,
}

impl Default for Held<'_> {
    fn default() -> Self {
        Self { signed: None, cookie: None, source: "10.0.0.1", origin: Some("https://ink.example"), site: None }
    }
}

fn json(reply: &Reply) -> Value {
    serde_json::from_slice(&reply.body).unwrap_or(Value::Null)
}

#[test]
fn a_link_signs_in_once_and_the_session_is_a_cookie() {
    let fixture = Fixture::new(Wanted::default());
    assert_eq!(fixture.ask("me@example.org", "10.0.0.1").status, Status::Accepted);
    let token = fixture.token();
    // looking does not spend the link
    let path = format!("/v1/login/{token}");
    let looked = fixture.send((Method::Get, &path), Held::default(), b"");
    assert_eq!((looked.status, json(&looked)["purpose"].clone()), (Status::Ok, json!("login")));
    let reply = fixture.redeem(&token);
    assert_eq!(reply.status, Status::Ok);
    let body = json(&reply);
    assert_eq!(body["created_account"], json!(true));
    let secret = body["session"].as_str().unwrap().to_owned();
    assert_eq!(secret.len(), 64);
    // the cookie: name, value and every attribute
    let cookie = reply.cookie.as_ref().unwrap().to_string();
    assert!(cookie.starts_with(&format!("ink-session={secret}; ")), "{cookie}");
    for attribute in ["Path=/", "HttpOnly", "SameSite=Lax"] {
        assert!(cookie.contains(attribute), "{cookie}");
    }
    assert!(!cookie.contains("Secure"), "a local instance is plain http: {cookie}");
    // the link is spent
    let again = fixture.redeem(&token);
    assert_eq!(again.status, Status::NotFound);
    assert_eq!(fixture.send((Method::Get, &path), Held::default(), b"").status, Status::NotFound);
    // the session is a cookie: the API host reads signatures and no cookie, and this reads cookies
    let jar = format!("a=b; ink-session={secret}");
    let held = Held { cookie: Some(&jar), ..Held::default() };
    let shown = fixture.send((Method::Get, "/v1/session"), held, b"");
    assert_eq!(shown.status, Status::Ok);
    let bearer = format!("Bearer {secret}");
    let as_header = fixture.send((Method::Get, "/v1/session"), Held { signed: Some(&bearer), ..Held::default() }, b"");
    assert_eq!(as_header.status, Status::Unauthorized, "a session is not a bearer token");
    let sites = fixture.send((Method::Get, "/v1/sites"), held, b"");
    assert_eq!(sites.status, Status::BadRequest, "the API is signed, never cookied");
    // ending it ends it, and clears the cookie
    let end = fixture.send((Method::Delete, "/v1/session"), held, b"");
    assert_eq!(end.status, Status::Ok);
    assert!(end.cookie.as_ref().unwrap().to_string().contains("Max-Age=0"));
    let after = fixture.send((Method::Get, "/v1/session"), held, b"");
    assert_eq!((after.status, after.challenge), (Status::Unauthorized, true));
}

#[test]
fn a_person_with_a_session_approves_a_key_and_a_stranger_cannot_guess_the_code() {
    let fixture = Fixture::new(Wanted::default());
    let secret = fixture.sign_in("me@example.org");
    let jar = format!("ink-session={secret}");
    let held = Held { cookie: Some(&jar), ..Held::default() };
    let device = sig::Signer::from_seed(&sig::Seed::new([3; 32]));
    let ask = json!({ "key": device.public().to_string(), "label": "laptop", "purpose": "device" }).to_string();
    let asked = fixture.send((Method::Post, "/v1/enrol"), Held::default(), ask.as_bytes());
    assert_eq!(asked.status, Status::Created, "{}", String::from_utf8_lossy(&asked.body));
    let code = json(&asked)["user_code"].as_str().unwrap().to_owned();
    let path = format!("/v1/approve/{code}");
    // no session, no look: a signature does not stand in for the cookie
    assert_eq!(fixture.send((Method::Get, &path), Held::default(), b"").status, Status::Unauthorized);
    let signed = Held { signed: Some("Ink-Sig-1 k=x"), ..Held::default() };
    assert_eq!(fixture.send((Method::Get, &path), signed, b"").status, Status::Unauthorized);
    let seen = fixture.send((Method::Get, &path), held, b"");
    assert_eq!(seen.status, Status::Ok);
    let seen = json(&seen);
    assert_eq!(
        (seen["label"].clone(), seen["fingerprint"].clone()),
        (json!("laptop"), json!(device.id().fingerprint().to_string()))
    );
    // the fingerprint compared must be the one asked
    let wrong = json!({ "fingerprint": "AAAA-AAAA-AA" }).to_string();
    assert_eq!(fixture.send((Method::Post, &path), held, wrong.as_bytes()).status, Status::Conflict);
    let right = json!({ "fingerprint": device.id().fingerprint().to_string() }).to_string();
    assert_eq!(fixture.send((Method::Post, &path), held, right.as_bytes()).status, Status::Ok);
    // spent: the answer of a code that never was
    let spent = fixture.send((Method::Post, &path), held, right.as_bytes());
    assert_eq!(spent.status, Status::NotFound);
    // the spent code was one miss; four more codes that name nothing close the endpoint for a minute
    for letter in ["B", "C", "D", "F"] {
        let guess = format!("/v1/approve/{0}-{0}", letter.repeat(4));
        assert_eq!(fixture.send((Method::Get, &guess), held, b"").status, Status::NotFound);
    }
    let closed = fixture.send((Method::Get, "/v1/approve/HHHH-HHHH"), held, b"");
    assert_eq!((closed.status, closed.retry), (Status::TooMany, Some(Duration::from_secs(60))));
    fixture.clock.advance(Duration::from_secs(61));
    assert_eq!(fixture.send((Method::Get, "/v1/approve/HHHH-HHHH"), held, b"").status, Status::NotFound);
}

#[test]
fn a_secure_instance_sets_a_secure_host_cookie() {
    let fixture = Fixture::new(Wanted { secure: true, ..Wanted::default() });
    assert_eq!(fixture.ask("me@example.org", "10.0.0.1").status, Status::Accepted);
    let reply = fixture.redeem(&fixture.token());
    let cookie = reply.cookie.unwrap().to_string();
    assert!(cookie.starts_with("__Host-ink-session="), "{cookie}");
    assert!(cookie.contains("; Secure"), "{cookie}");
}

#[test]
fn the_reply_to_a_request_is_the_same_whoever_is_asked_about() {
    let fixture = Fixture::new(Wanted::default());
    fixture.sign_in("me@example.org");
    let known = fixture.ask("me@example.org", "10.0.0.2");
    let unknown = fixture.ask("nobody@example.org", "10.0.0.3");
    let shouty = fixture.ask("ME@Example.ORG", "10.0.0.4");
    assert_eq!(known, unknown);
    assert_eq!(known, shouty);
    assert_eq!(known.status, Status::Accepted);
    assert_eq!(json(&known), json!({ "requested": true }));
    // mail went to the one that has an account, and only to it
    pollster::block_on(fixture.courier.step(&Heard::default()));
    let sent = fixture.mail.sent();
    let to: Vec<&str> = sent.iter().map(|message| message.to.as_str()).collect();
    assert_eq!(to, ["me@example.org", "me@example.org", "me@example.org"]);
}

#[test]
fn a_link_that_never_was_ended_or_changed_is_one_answer() {
    let fixture = Fixture::new(Wanted::default());
    assert_eq!(fixture.ask("me@example.org", "10.0.0.1").status, Status::Accepted);
    let token = fixture.token();
    let mut changed = token.clone().into_bytes();
    changed[10] = if changed[10] == b'a' { b'b' } else { b'a' };
    let changed = String::from_utf8(changed).unwrap();
    let never = "0".repeat(64);
    let malformed = fixture.redeem("short");
    let replies = [fixture.redeem(&changed), fixture.redeem(&never), malformed];
    for reply in &replies {
        assert_eq!(reply.status, Status::NotFound);
        assert_eq!(reply, &replies[0]);
        assert!(reply.cookie.is_none());
    }
    // it ends: the same answer after its lifetime
    fixture.clock.advance(Settings::default().login.duration());
    assert_eq!(fixture.redeem(&token), replies[0]);
}

#[test]
fn asking_is_limited_per_address_and_per_source_whether_or_not_the_address_exists() {
    let rates = Rates { address: "2/1h".parse().unwrap(), source: "3/1h".parse().unwrap(), tracked: 100 };
    let fixture = Fixture::new(Wanted { rates, ..Wanted::default() });
    assert_eq!(fixture.ask("nobody@example.org", "10.0.0.1").status, Status::Accepted);
    assert_eq!(fixture.ask("nobody@example.org", "10.0.0.2").status, Status::Accepted);
    let limited = fixture.ask("nobody@example.org", "10.0.0.3");
    assert_eq!(limited.status, Status::TooMany);
    assert_eq!(limited.retry, Some(Duration::from_hours(1)));
    assert_eq!(json(&limited)["problem"], json!("rate-limited"));
    // the source: three requests, and a fourth for another address is turned away
    assert_eq!(fixture.ask("a@example.org", "10.1.0.1").status, Status::Accepted);
    assert_eq!(fixture.ask("b@example.org", "10.1.0.1").status, Status::Accepted);
    assert_eq!(fixture.ask("c@example.org", "10.1.0.1").status, Status::Accepted);
    assert_eq!(fixture.ask("d@example.org", "10.1.0.1").status, Status::TooMany);
    // and the clock frees them
    fixture.clock.advance(Duration::from_hours(1) + Duration::from_secs(1));
    assert_eq!(fixture.ask("nobody@example.org", "10.0.0.3").status, Status::Accepted);
}

#[test]
fn a_bad_address_or_body_is_a_400_before_anything_is_counted() {
    let fixture = Fixture::new(Wanted::default());
    for body in [&b"{}"[..], b"not json", br#"{"address":"nope"}"#, br#"{"address":"a@b.org","extra":1}"#] {
        let reply = fixture.send((Method::Post, "/v1/login"), Held::default(), body);
        assert_eq!(reply.status, Status::BadRequest, "{}", String::from_utf8_lossy(body));
    }
    let wrong = fixture.send((Method::Get, "/v1/login"), Held::default(), b"");
    assert_eq!((wrong.status, wrong.allow.clone()), (Status::MethodNotAllowed, vec![Method::Post]));
}

#[test]
fn the_first_run_link_signs_the_owner_in_once() {
    let fixture = Fixture::new(Wanted::default());
    let drawn = control::signin::Drawn::new([[200; 16], [201; 16], [202; 16], [203; 16], [204; 16]]);
    let issued = pollster::block_on(fixture.entry.admin(None, drawn, fixture.clock_now())).unwrap().unwrap();
    let token = issued.proof.reveal().to_owned();
    let path = format!("/v1/login/{token}");
    let body = json!({ "address": "owner@example.org" }).to_string();
    let reply = fixture.send((Method::Post, &path), Held::default(), body.as_bytes());
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(json(&reply)["created_account"], json!(true));
    let again = fixture.send((Method::Post, &path), Held::default(), body.as_bytes());
    assert_eq!(again.status, Status::NotFound);
    // the owner signs in by mail afterwards
    fixture.sign_in("owner@example.org");
}

impl Fixture {
    fn clock_now(&self) -> Timestamp {
        clock::Clock::now(&self.clock)
    }
}

#[test]
fn the_sign_up_policy_decides_whether_a_new_address_gets_mail() {
    let settings = Settings { signup: Signup::Closed, ..Settings::default() };
    let fixture = Fixture::new(Wanted { settings, ..Wanted::default() });
    let reply = fixture.ask("new@example.org", "10.0.0.1");
    assert_eq!(reply.status, Status::Accepted);
    pollster::block_on(fixture.courier.step(&Heard::default()));
    assert_eq!(fixture.mail.sent(), vec![]);
}

#[test]
fn a_get_signs_in_only_when_the_operator_says_so() {
    let fixture = Fixture::new(Wanted { reveals: true, ..Wanted::default() });
    assert_eq!(fixture.ask("me@example.org", "10.0.0.1").status, Status::Accepted);
    let token = fixture.token();
    let reply = fixture.send((Method::Get, &format!("/v1/login/{token}")), Held::default(), b"");
    assert_eq!(reply.status, Status::Ok);
    assert!(reply.cookie.is_some());
}

#[test]
fn a_full_outbox_drops_and_the_courier_says_so() {
    let fixture = Fixture::new(Wanted { capacity: 1, ..Wanted::default() });
    // the first account makes the policy "first" close, so use one address that signs in
    fixture.sign_in("me@example.org");
    assert_eq!(fixture.ask("me@example.org", "10.0.0.2").status, Status::Accepted);
    assert_eq!(fixture.ask("me@example.org", "10.0.0.3").status, Status::Accepted);
    assert_eq!(fixture.outbox.dropped(), 1);
    let heard = Heard::default();
    pollster::block_on(fixture.courier.step(&heard));
    let notes = heard.0.lock().unwrap().clone();
    assert_eq!(notes, ["Sent", "Dropped(1)"]);
}

#[test]
fn nothing_that_prints_holds_a_secret() {
    let fixture = Fixture::new(Wanted::default());
    assert_eq!(fixture.ask("me@example.org", "10.0.0.1").status, Status::Accepted);
    let token = fixture.token();
    let path = format!("/v1/login/{token}");
    let request = Request {
        method: Method::Post,
        path: path.clone(),
        query: None,
        authorization: Some(b"Bearer sekrit".to_vec()),
        cookie: Some(b"ink-session=sekrit".to_vec()),
        origin: Some("https://ink.example".to_owned()),
        site: None,
        source: None,
        body: Payload::Whole(vec![]),
    };
    let shown = format!("{request:?}");
    assert!(!shown.contains(&token) && !shown.contains("sekrit"), "{shown}");
    let reply = pollster::block_on(fixture.front.answer(&request));
    let secret = json(&reply)["session"].as_str().unwrap().to_owned();
    let shown = format!("{reply:?}");
    assert!(!shown.contains(&secret), "{shown}");
    assert_eq!(edge_api::Enter::redact(&path), "/v1/login/<redacted>");
    // a refusal says nothing of the token either
    let refused = fixture.redeem(&token);
    assert!(!String::from_utf8_lossy(&refused.body).contains(&token));
    let message = fixture.mail.sent();
    assert!(!format!("{:?}", message[0]).contains(&token), "a message prints its link");
}

fn code(reply: &Reply) -> String {
    json(reply)["problem"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn an_account_lists_its_sessions_and_ends_them_by_id_and_no_one_elses() {
    let settings = Settings { signup: Signup::Open, ..Settings::default() };
    let fixture = Fixture::new(Wanted { settings, ..Wanted::default() });
    let (first, second) = (fixture.sign_in("me@example.org"), fixture.sign_in("me@example.org"));
    let stranger = fixture.sign_in("you@example.org");
    let (one, two, other) =
        (format!("ink-session={first}"), format!("ink-session={second}"), format!("ink-session={stranger}"));
    let (one, two, other) = (
        Held { cookie: Some(&one), ..Held::default() },
        Held { cookie: Some(&two), ..Held::default() },
        Held { cookie: Some(&other), ..Held::default() },
    );
    // anonymous: no list, and the cookie is not required to be sent for the answer to be a 401
    assert_eq!(fixture.send((Method::Get, "/v1/sessions"), Held::default(), b"").status, Status::Unauthorized);
    let listed = fixture.send((Method::Get, "/v1/sessions"), one, b"");
    assert_eq!(listed.status, Status::Ok);
    let list = json(&listed);
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 2, "the stranger's session is not listed");
    assert_eq!(rows.iter().filter(|row| row["current"] == json!(true)).count(), 1);
    let text = String::from_utf8_lossy(&listed.body).into_owned();
    assert!(!text.contains(&first) && !text.contains(&second), "a list has no secret: {text}");
    let theirs = rows.iter().find(|row| row["current"] == json!(false)).unwrap()["id"].as_str().unwrap().to_owned();
    // the stranger cannot end it, and gets the answer of an id that names nothing
    let denied = fixture.send((Method::Delete, &format!("/v1/sessions/{theirs}")), other, b"");
    assert_eq!((denied.status, code(&denied)), (Status::NotFound, "auth.session.missing".to_owned()));
    assert_eq!(fixture.send((Method::Get, "/v1/session"), two, b"").status, Status::Ok);
    // the owner ends the other one, and it stops working at once
    let ended = fixture.send((Method::Delete, &format!("/v1/sessions/{theirs}")), one, b"");
    assert_eq!((ended.status, ended.cookie.is_none()), (Status::Ok, true));
    assert_eq!(fixture.send((Method::Get, "/v1/session"), two, b"").status, Status::Unauthorized);
    assert_eq!(json(&fixture.send((Method::Get, "/v1/sessions"), one, b"")).as_array().unwrap().len(), 1);
    // ended, unknown and malformed ids are one answer
    for path in [format!("/v1/sessions/{theirs}"), format!("/v1/sessions/{}", "0".repeat(32)), "/v1/sessions/x".to_owned()] {
        let gone = fixture.send((Method::Delete, &path), one, b"");
        assert_eq!((gone.status, code(&gone)), (Status::NotFound, "auth.session.missing".to_owned()), "{path}");
    }
    // ending the asking one clears its cookie
    let current = json(&fixture.send((Method::Get, "/v1/sessions"), one, b""))[0]["id"].as_str().unwrap().to_owned();
    let farewell = fixture.send((Method::Delete, &format!("/v1/sessions/{current}")), one, b"");
    assert!(farewell.cookie.unwrap().to_string().contains("Max-Age=0"));
    assert_eq!(fixture.send((Method::Get, "/v1/sessions"), one, b"").status, Status::Unauthorized);
    // the stranger kept theirs
    assert_eq!(fixture.send((Method::Get, "/v1/sessions"), other, b"").status, Status::Ok);
    // a method the route does not take lists the ones it does
    let wrong = fixture.send((Method::Post, "/v1/sessions"), other, b"");
    assert_eq!((wrong.status, wrong.allow), (Status::MethodNotAllowed, vec![Method::Get]));
}

#[test]
fn a_state_change_needs_the_dashboards_origin_or_same_origin() {
    let fixture = Fixture::new(Wanted::default());
    let secret = fixture.sign_in("me@example.org");
    let jar = format!("ink-session={secret}");
    let me = Held { cookie: Some(&jar), ..Held::default() };
    let asked = json!({ "address": "me@example.org" }).to_string();
    assert_eq!(fixture.send((Method::Post, "/v1/login"), Held::default(), asked.as_bytes()).status, Status::Accepted);
    let token = fixture.token();
    let link = format!("/v1/login/{token}");
    let device = sig::Signer::from_seed(&sig::Seed::new([3; 32]));
    let ask = json!({ "key": device.public().to_string(), "label": "laptop", "purpose": "device" }).to_string();
    let enrol = fixture.send((Method::Post, "/v1/enrol"), Held::default(), ask.as_bytes());
    let approve = format!("/v1/approve/{}", json(&enrol)["user_code"].as_str().unwrap());
    let sessions = json(&fixture.send((Method::Get, "/v1/sessions"), me, b""));
    let id = format!("/v1/sessions/{}", sessions[0]["id"].as_str().unwrap());
    let fingerprint = json!({ "fingerprint": device.id().fingerprint().to_string() }).to_string();
    let routes: [(Method, &str, &[u8]); 6] = [
        (Method::Post, "/v1/login", asked.as_bytes()),
        (Method::Post, &link, b""),
        (Method::Delete, "/v1/session", b""),
        (Method::Delete, &id, b""),
        (Method::Post, &approve, fingerprint.as_bytes()),
        (Method::Delete, &approve, b""),
    ];
    let elsewhere = [
        (Some("https://evil.example"), None),
        (Some("https://evil.example"), Some("same-origin")),
        (Some("https://ink.example:8443"), None),
        (Some("null"), None),
        (None, Some("cross-site")),
        (None, Some("same-site")),
        (None, None),
    ];
    for (method, path, body) in routes {
        for (origin, site) in elsewhere {
            let held = Held { origin, site, ..me };
            let reply = fixture.send((method, path), held, body);
            let shown = (method, origin, site);
            assert_eq!((reply.status, code(&reply)), (Status::Forbidden, "auth.origin.refused".to_owned()), "{shown:?}");
            assert!(reply.cookie.is_none(), "a refused request sets nothing");
        }
    }
    // nothing was changed: the link still works, the session is alive and the enrolment is pending
    assert_eq!(fixture.send((Method::Get, "/v1/session"), me, b"").status, Status::Ok);
    assert_eq!(fixture.send((Method::Get, &link), Held::default(), b"").status, Status::Ok);
    assert_eq!(fixture.send((Method::Get, &approve), me, b"").status, Status::Ok);
    // a read needs no origin, and a browser that says same-origin and no more is taken
    let bare = Held { origin: None, site: None, ..me };
    assert_eq!(fixture.send((Method::Get, "/v1/sessions"), bare, b"").status, Status::Ok);
    let same = Held { origin: None, site: Some("same-origin"), ..me };
    assert_eq!(fixture.send((Method::Post, &approve), same, fingerprint.as_bytes()).status, Status::Ok);
    assert_eq!(fixture.send((Method::Delete, "/v1/session"), same, b"").status, Status::Ok);
}

#[test]
fn a_dashboard_on_another_host_is_added_by_the_operator_and_a_get_that_signs_in_is_theirs_too() {
    let dashboard = Some("https://App.Dash.example");
    let fixture = Fixture::new(Wanted { reveals: true, dashboard, ..Wanted::default() });
    let ask = json!({ "address": "you@example.org" }).to_string();
    for (origin, status) in [
        ("https://app.dash.example", Status::Accepted),
        ("https://ink.example", Status::Accepted),
        ("https://dash.example", Status::Forbidden),
    ] {
        let held = Held { origin: Some(origin), source: "10.0.0.7", ..Held::default() };
        assert_eq!(fixture.send((Method::Post, "/v1/login"), held, ask.as_bytes()).status, status, "{origin}");
    }
    assert_eq!(fixture.ask("me@example.org", "10.0.0.1").status, Status::Accepted);
    let token = fixture.token();
    // `reveals`: a GET signs in, with no browser to ask, so it is not held to an origin
    let bare = Held { origin: None, site: None, ..Held::default() };
    assert_eq!(fixture.send((Method::Get, &format!("/v1/login/{token}")), bare, b"").status, Status::Ok);
}
