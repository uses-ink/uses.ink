//! A signing rig: the API on real adapters (SQLite in memory, a directory, the queue in SQLite
//! rows, a sealer with a fresh key), a manual clock and counting ids, an operator key in a file
//! and a way to sign any request with any key.

#![allow(dead_code, unreachable_pub, reason = "each test file uses some of a rig that is a private module")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use blob_dir::Directory;
use clock::{Manual, System};
use control::keys::{Rights, Settings as KeySettings};
use control::{AccountKind, Control, Domain, Id, NewAccount, Origin, Remembered};
use db::Steps;
use db_sqlite::Sqlite;
use edge_api::{
    Api, Closing, Enrol, Limits, Method, OperatorFile, OperatorKey, Payload, Reply, Request, Signatures, Signing,
    Window,
};
use jiff::Timestamp;
use queue_local::{Local as Queue, Queued};
use secret_local::Local as Sealer;
use serde_json::Value;
use sig::{Audience, Created, KeyType, Nonce, Parts, Seed, Signer};

pub type Store = Arc<Sqlite>;
pub type Ports = (Store, Directory, Queue<Store>, Sealer);
pub type Callers = Signatures<Store, &'static Manual, OperatorFile>;
pub type Wired = Api<Ports, Callers, &'static Manual, System>;

pub const AUDIENCE: &str = "api.example";
pub const SKEW: u64 = 5;

pub fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

pub fn signer(n: u8) -> Signer {
    Signer::from_seed(&Seed::new([n; 32]))
}

/// What one test runs against.
pub struct Rig {
    pub api: Wired,
    pub clock: &'static Manual,
    pub enrol: Enrol<Store>,
    pub db: Store,
    pub operator: Signer,
    pub account: Id<AccountKind>,
    pub folder: tempfile::TempDir,
    nonces: AtomicU64,
}

impl Rig {
    pub fn new(limits: Limits) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let db: Store = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new()
            .with::<Domain>()
            .unwrap()
            .with::<Remembered>()
            .unwrap()
            .with::<Queued>()
            .unwrap()
            .with::<control::signin::Entry>()
            .unwrap()
            .with::<control::keys::Entry>()
            .unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (
            db.clone(),
            Directory::open(folder.path().join("blobs")).unwrap(),
            Queue::new(db.clone()),
            Sealer::ephemeral().unwrap(),
        );
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let control = Control::new(ports, origin);
        let account = Id::from_bytes([9; 16]);
        pollster::block_on(control.add_account(NewAccount { id: account, at: start() })).unwrap();
        let clock: &'static Manual = Box::leak(Box::new(Manual::new(start(), 0)));
        let operator = signer(100);
        let file = OperatorFile::at(folder.path().join("operator-keys"));
        let line = format!("ed25519-v1 {} operator", operator.public());
        file.add(&OperatorKey::parse(&line).unwrap().remove(0)).unwrap();
        let enrol = Enrol::new(db.clone(), KeySettings::default(), Closing::default());
        let mut config = Signing::new(Audience::parse(AUDIENCE).unwrap(), account);
        config.window = Window::new(SKEW).unwrap();
        let signatures = Signatures::new(enrol.keys().clone(), (clock, file), config);
        let api = Api::new(control, (signatures, enrol.clone()), (clock, System), limits);
        let rig = Self { api, clock, enrol, db, operator, account, folder, nonces: AtomicU64::new(1) };
        // the boot barrier: no signed request is taken until a request made inside the window can have been made after the barrier
        rig.clock.advance(Duration::from_secs(2 * SKEW + 1));
        rig
    }

    pub fn now(&self) -> u64 {
        use clock::Clock as _;
        u64::try_from(self.clock.now().as_second()).unwrap()
    }

    pub fn nonce(&self) -> Nonce {
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&self.nonces.fetch_add(1, Ordering::Relaxed).to_be_bytes());
        Nonce::new(bytes)
    }

    /// The header of `method` `target` with `body`, made by `by` at `created`, with `nonce`.
    pub fn header(by: &Signer, (method, target): (Method, &str), body: &[u8], when: (u64, Nonce)) -> Vec<u8> {
        let method = sig::Method::parse(&method.to_string()).unwrap();
        let target = sig::Target::parse(target).unwrap();
        let audience = Audience::parse(AUDIENCE).unwrap();
        let parts = Parts { audience: &audience, method: &method, target: &target, body };
        by.sign(&parts, Created::new(when.0), when.1).to_string().into_bytes()
    }

    /// A request signed by `by` now.
    pub fn request(&self, by: &Signer, route: (Method, &str), body: &[u8]) -> Request {
        let authorization = Self::header(by, route, body, (self.now(), self.nonce()));
        Self::unsigned(route, Some(authorization), body)
    }

    /// A request with this `Authorization` value, or none.
    pub fn unsigned((method, target): (Method, &str), authorization: Option<Vec<u8>>, body: &[u8]) -> Request {
        let (path, query) = match target.split_once('?') {
            Some((path, query)) => (path, Some(query.to_owned())),
            None => (target, None),
        };
        Request {
            method,
            path: path.to_owned(),
            query,
            authorization,
            cookie: None,
            origin: None,
            site: None,
            source: None,
            body: Payload::Whole(body.to_vec()),
        }
    }

    pub fn answer(&self, request: &Request) -> Reply {
        pollster::block_on(self.api.respond(request))
    }

    /// A call signed by `by`, with its JSON answer.
    #[allow(clippy::too_many_arguments, reason = "a test helper: who, verb, target and body read best flat")]
    pub fn call_as(&self, by: &Signer, method: Method, target: &str, body: &[u8]) -> (Reply, Value) {
        let reply = self.answer(&self.request(by, (method, target), body));
        let value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
        (reply, value)
    }

    /// A call as the operator.
    pub fn call(&self, method: Method, target: &str, body: &[u8]) -> (Reply, Value) {
        self.call_as(&self.operator, method, target, body)
    }

    /// Enrols `who` and returns the ask's id, without approving it. `extra` is merged into the
    /// body: `{"purpose": "device"}` when it has none.
    pub fn ask(&self, who: &Signer, extra: &Value) -> (String, String) {
        let mut body = serde_json::json!({ "key": who.public().to_string(), "label": "test key", "purpose": "device" });
        for (name, value) in extra.as_object().unwrap() {
            body[name] = value.clone();
        }
        let reply = self.answer(&Self::unsigned((Method::Post, "/v1/enrol"), None, body.to_string().as_bytes()));
        assert_eq!(reply.status.code(), 201, "{}", String::from_utf8_lossy(&reply.body));
        let asked: Value = serde_json::from_slice(&reply.body).unwrap();
        (asked["id"].as_str().unwrap().to_owned(), asked["user_code"].as_str().unwrap().to_owned())
    }

    /// Enrols `who` as a key of the account, approved with every right the person has.
    pub fn enrolled(&self, who: &Signer, extra: &Value) -> String {
        let (id, code) = self.ask(who, extra);
        let approval = control::keys::Approval {
            code: code.parse().unwrap(),
            fingerprint: who.id().fingerprint(),
            by: self.account,
            within: Rights::EVERY,
        };
        pollster::block_on(self.enrol.keys().approve(approval, self.clock_now())).unwrap();
        id
    }

    fn clock_now(&self) -> Timestamp {
        use clock::Clock as _;
        self.clock.now()
    }

    pub fn key_type() -> KeyType {
        KeyType::Ed25519V1
    }
}
