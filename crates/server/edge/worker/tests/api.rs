//! The API the Worker wires (`Hosting`, `Wired`), over the native adapters under it: SQLite rows
//! and a directory of blobs. What differs on a Worker is the adapters and the nonce store, and
//! `cargo xtask worker app` runs the whole product in workerd.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_edge_worker as edge_worker;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use blob_dir::Directory;
use clock::{Clock as _, Manual, System};
use db_sqlite::Sqlite;
use edge_api::{Answers, Method, Payload, Remembered, Reply, Request};
use edge_worker::{Hosting, Migrations, Unready, Vars, Wired};
use jiff::Timestamp;
use queue_rows::Local as Queue;
use secret_local::Local as Sealer;
use serde_json::Value;
use sig::{Audience, Created, Nonce, Parts, Seed, Signer};

type Store = Arc<Sqlite>;
type Ports = Arc<(Store, Directory, Queue<Store>, Sealer)>;

/// Variables as a map.
struct Map(BTreeMap<&'static str, String>);

impl Vars for Map {
    fn var(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

const AUDIENCE: &str = "api.example.test";

fn signer() -> Signer {
    Signer::from_seed(&Seed::new([100; 32]))
}

fn variables() -> Map {
    let line = format!("ed25519-v1 {} operator", signer().public());
    Map(BTreeMap::from([
        ("ORIGIN", "https://example.test".to_owned()),
        ("API_HOST", AUDIENCE.to_owned()),
        ("OPERATOR_KEYS", line),
        ("AUTH_SKEW", "5".to_owned()),
    ]))
}

/// What one test runs against: the API, its clock, and the folder the blobs are in.
struct Rig {
    api: Wired<Ports, Remembered, &'static Manual, System>,
    clock: &'static Manual,
    _folder: tempfile::TempDir,
}

impl Rig {
    fn new(vars: &Map) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let db: Store = Arc::new(Sqlite::memory().unwrap());
        pollster::block_on(Migrations::steps().unwrap().apply(&db)).unwrap();
        let ports: Ports = Arc::new((
            db.clone(),
            Directory::open(folder.path().join("blobs")).unwrap(),
            Queue::new(db.clone()),
            Sealer::ephemeral().unwrap(),
        ));
        let clock: &'static Manual =
            Box::leak(Box::new(Manual::new(Timestamp::from_second(1_800_000_000).unwrap(), 0)));
        let api =
            Hosting::read(vars).unwrap().wire((ports, db), (Remembered::new(), Remembered::new()), (clock, System));
        Self { api, clock, _folder: folder }
    }

    fn request(method: Method, target: &str, authorization: Option<Vec<u8>>) -> Request {
        Request {
            method,
            path: target.to_owned(),
            query: None,
            authorization,
            headers: Vec::new(),
            cookie: None,
            origin: None,
            site: None,
            source: None,
            body: Payload::Whole(Vec::new()),
        }
    }

    /// The header `by` signs `method` of `target` with, now, and `nonce`.
    fn header(&self, method: &str, target: &str, nonce: u8) -> Vec<u8> {
        let method = sig::Method::parse(method).unwrap();
        let target = sig::Target::parse(target).unwrap();
        let audience = Audience::parse(AUDIENCE).unwrap();
        let parts = Parts { audience: &audience, method: &method, target: &target, body: b"" };
        let created = Created::new(u64::try_from(self.clock.now().as_second()).unwrap());
        signer().sign(&parts, created, Nonce::new([nonce; 16])).to_string().into_bytes()
    }

    fn answer(&self, request: &Request) -> (Reply, Value) {
        let reply = pollster::block_on(self.api.answer(request));
        let value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
        (reply, value)
    }
}

#[test]
fn a_missing_variable_is_named() {
    let mut vars = variables();
    vars.0.remove("ORIGIN");
    assert!(matches!(Hosting::read(&vars), Err(Unready::Missing { name: "ORIGIN" })));
}

#[test]
fn an_unusable_variable_is_named_with_its_cause() {
    let mut vars = variables();
    vars.0.insert("UPLOAD_BYTES", "lots".to_owned());
    let error = Hosting::read(&vars).unwrap_err();
    assert!(matches!(error, Unready::Invalid { name: "UPLOAD_BYTES", .. }), "{error}");
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn the_limits_are_the_variables_over_the_workers_own() {
    let mut vars = variables();
    vars.0.insert("UPLOAD_FILES", "7".to_owned());
    let limits = Hosting::read(&vars).unwrap().limits();
    assert_eq!(limits.files, 7);
    assert!(limits.archive < edge_api::Limits::default().archive);
}

#[test]
fn an_unsigned_call_is_told_how_to_sign() {
    let rig = Rig::new(&variables());
    let (reply, body) = rig.answer(&Rig::request(Method::Get, "/v1/sites", None));
    assert_eq!(reply.status.code(), 400);
    assert_eq!(body["problem"], "auth.signature.malformed");
}

#[test]
fn what_the_worker_lacks_is_answered_as_pending() {
    let rig = Rig::new(&variables());
    let id = "01".repeat(16);
    for (method, path) in [
        (Method::Get, "/".to_owned()),
        (Method::Get, "/login/abc".to_owned()),
        (Method::Get, "/v1/passkeys".to_owned()),
        (Method::Post, format!("/v1/sites/{id}/builds")),
    ] {
        let (reply, body) = rig.answer(&Rig::request(method, &path, None));
        assert_eq!(reply.status.code(), 501, "{path}");
        assert_eq!(body["problem"], "worker.route.pending", "{path}");
        assert!(body["message"].as_str().unwrap().contains("planned"), "{path}");
    }
}

#[test]
fn a_signed_call_is_taken_once() {
    let rig = Rig::new(&variables());
    rig.clock.advance(Duration::from_secs(1));
    let header = rig.header("GET", "/v1/sites", 1);
    let (first, listed) = rig.answer(&Rig::request(Method::Get, "/v1/sites", Some(header.clone())));
    assert_eq!(first.status.code(), 200, "{listed}");
    let (again, refused) = rig.answer(&Rig::request(Method::Get, "/v1/sites", Some(header)));
    assert_ne!(again.status.code(), 200, "{refused}");
}
