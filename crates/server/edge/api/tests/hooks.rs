//! A forge's webhook on a control plane over real adapters: a signed push queues a build of the
//! site that proved the repository, a replayed delivery is refused from the nonce store, and every
//! refusal says as little as the forge crate does.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_edge_api as edge_api;

use std::sync::Arc;

use authority_forge::{Challenge, Limits as Forged, Proven, Repo};
use blob_dir::Directory;
use build::{Source, Trigger};
use clock::{Counter, Manual};
use control::{
    AccountKind, Binding, Control, Domain, Id, JobState, NewAccount, NewRepo, NewSite, Origin, Remembered, SiteKind,
    Treed,
};
use db::Steps;
use db_sqlite::Sqlite;
use edge_api::{Answers, Hooks, Method, Payload, Remembered as Seen, Request, Status};
use jiff::Timestamp;
use queue_rows::{Local as Queue, Queued};
use secret::Plain;
use secret_local::Local as Sealer;
use serde_json::Value;
use watch::Previews;

type Ports = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

const SECRET: &str = "It's a Secret to Everybody";
const PUSH: &[u8] = include_bytes!("../../../authority/github/tests/fixtures/push.json");
const PUSH_SIG: &str = include_str!("../../../authority/github/tests/fixtures/push.json.sig");
const OTHER: &[u8] = include_bytes!("../../../authority/github/tests/fixtures/push-other.json");
const OTHER_SIG: &str = include_str!("../../../authority/github/tests/fixtures/push-other.json.sig");
const PING: &[u8] = include_bytes!("../../../authority/github/tests/fixtures/ping.json");
const PING_SIG: &str = include_str!("../../../authority/github/tests/fixtures/ping.json.sig");

fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

struct World {
    hooks: Hooks<Ports, Seen, &'static Manual, Counter>,
    site: Id<SiteKind>,
    _folder: tempfile::TempDir,
}

impl World {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = control::Schema::after(
            Steps::new()
                .with::<Domain>()
                .unwrap()
                .with::<Remembered>()
                .unwrap()
                .with::<Treed>()
                .unwrap()
                .with::<Queued>()
                .unwrap(),
        )
        .unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (
            db.clone(),
            Directory::open(folder.path().join("b")).unwrap(),
            Queue::new(db),
            Sealer::ephemeral().unwrap(),
        );
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let control = Control::new(ports, origin);
        let account: Id<AccountKind> = Id::from_bytes([1; 16]);
        pollster::block_on(control.add_account(NewAccount { id: account, at: start() })).unwrap();
        let source = Source::Git { url: "https://github.com/octo/docs".parse().unwrap(), revision: None };
        let site = Id::from_bytes([2; 16]);
        let new = NewSite { id: site, account, handle: "journal".parse().unwrap(), source, at: start() };
        pollster::block_on(control.add_site(new)).unwrap();
        let repo: Repo = "github.com/octo/docs".parse().unwrap();
        let asking = NewRepo {
            site,
            repo: repo.clone(),
            challenge: Challenge::from_bytes([3; 16]),
            secret: Plain::from(SECRET),
            credential: Id::from_bytes([4; 16]),
            at: start(),
        };
        pollster::block_on(control.claim_repo(asking)).unwrap();
        pollster::block_on(control.bind_repo(Binding { site, proof: Proven::of(repo), at: start() })).unwrap();
        let clock: &'static Manual = Box::leak(Box::new(Manual::new(start(), 0)));
        let hooks = Hooks::new(control, Seen::new(), (clock, Counter::new()), (Forged::DEFAULT, Previews(0)));
        Self { hooks, site, _folder: folder }
    }

    fn deliver(&self, forge: &str, headers: &[(&str, &str)], body: &[u8]) -> (Status, Value) {
        let request = Request {
            method: Method::Post,
            path: format!("/v1/hooks/{forge}"),
            query: None,
            authorization: None,
            cookie: None,
            origin: None,
            site: None,
            headers: headers.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect(),
            source: None,
            body: Payload::Whole(body.to_vec()),
        };
        let reply = pollster::block_on(self.hooks.answer(&request));
        (reply.status, serde_json::from_slice(&reply.body).unwrap_or(Value::Null))
    }

    fn push(&self, id: &str, body: &[u8], signature: &str) -> (Status, Value) {
        self.deliver(
            "github",
            &[("X-GitHub-Event", "push"), ("X-GitHub-Delivery", id), ("X-Hub-Signature-256", signature.trim())],
            body,
        )
    }

    fn jobs(&self) -> Vec<control::Job> {
        pollster::block_on(self.hooks.control().jobs(self.site, 10)).unwrap()
    }
}

#[test]
fn a_signed_push_queues_a_build_and_a_replay_of_the_delivery_is_refused() {
    let world = World::new();
    let (status, told) = world.push("d-1", PUSH, PUSH_SIG);
    assert_eq!((status, told["outcome"].as_str()), (Status::Accepted, Some("queued")), "{told}");
    let jobs = world.jobs();
    assert_eq!((jobs.len(), jobs[0].trigger, jobs[0].state), (1, Trigger::Push, JobState::Queued));
    // the same delivery id again: refused from the store, nothing queued
    let (status, problem) = world.push("d-1", PUSH, PUSH_SIG);
    assert_eq!((status, problem["problem"].as_str()), (Status::Conflict, Some("hook.delivery.replayed")));
    // a new id for the same commit is taken and queues nothing more
    let (status, told) = world.push("d-2", PUSH, PUSH_SIG);
    assert_eq!((status, told["outcome"].as_str()), (Status::Accepted, Some("duplicate")), "{told}");
    assert_eq!(world.jobs().len(), 1);
}

#[test]
fn a_delivery_that_is_not_what_it_says_is_refused_the_same_way() {
    let world = World::new();
    let bad = world.push("d-1", PUSH, "sha256=00");
    assert_eq!((bad.0, bad.1["problem"].as_str()), (Status::Unauthorized, Some("hook.signature.invalid")));
    // a repository no site proved answers as a bad signature does
    let stranger = world.push("d-2", OTHER, OTHER_SIG);
    assert_eq!((stranger.0, stranger.1["problem"].as_str()), (Status::Unauthorized, Some("hook.signature.invalid")));
    let mut tampered = PUSH.to_vec();
    tampered.push(b' ');
    assert_eq!(world.push("d-3", &tampered, PUSH_SIG).0, Status::Unauthorized);
    assert_eq!(world.jobs().len(), 0);
    // a signature refused does not use up the delivery id
    assert_eq!(world.push("d-1", PUSH, PUSH_SIG).0, Status::Accepted);
}

#[test]
fn a_ping_is_taken_and_ignored_and_the_route_has_its_own_manners() {
    let world = World::new();
    let ping = world.deliver(
        "github",
        &[("X-GitHub-Event", "ping"), ("X-GitHub-Delivery", "p-1"), ("X-Hub-Signature-256", PING_SIG.trim())],
        PING,
    );
    assert_eq!((ping.0, ping.1["outcome"].as_str()), (Status::Accepted, Some("ignored")), "{}", ping.1);
    assert_eq!(world.deliver("nowhere", &[], b"{}").0, Status::NotFound);
    let request = Request {
        method: Method::Get,
        path: "/v1/hooks/github".to_owned(),
        query: None,
        authorization: None,
        cookie: None,
        origin: None,
        site: None,
        headers: Vec::new(),
        source: None,
        body: Payload::Whole(Vec::new()),
    };
    assert_eq!(pollster::block_on(world.hooks.answer(&request)).status, Status::MethodNotAllowed);
    let over = Request { method: Method::Post, body: Payload::Over { limit: 5 }, ..request };
    assert_eq!(pollster::block_on(world.hooks.answer(&over)).status, Status::TooLarge);
    assert_eq!(world.deliver("github", &[], b"not json").0, Status::BadRequest);
}
