//! The API on real adapters: SQLite in memory, a directory in a throwaway folder, the queue in
//! SQLite rows and a sealer with a fresh key. Time is a manual clock and ids count.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_edge_api as edge_api;

use std::sync::Arc;

use blob_dir::Directory;
use clock::{Counter, Manual};
use control::{AccountKind, Control, Domain, Id, NewAccount, Origin};
use db::Steps;
use db_sqlite::Sqlite;
use edge_api::tar::Writer;
use edge_api::{Api, Limits, Method, Owner, Payload, Reply, Request, Status, Token};
use jiff::Timestamp;
use model::{Hash, Manifest, MediaType, Route, Served};
use queue_local::{Local as Queue, Queued};
use secret_local::Local as Sealer;
use serde_json::{Value, json};

type Ports = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);
type Wired = Api<Ports, Owner, Manual, Counter>;

/// An API with an owner, and the folder its blobs are in.
struct Fixture {
    api: Wired,
    token: String,
    account: Id<AccountKind>,
    _folder: tempfile::TempDir,
}

fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

impl Fixture {
    fn new(limits: Limits) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new().with::<Domain>().unwrap().with::<Queued>().unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (
            db.clone(),
            Directory::open(folder.path().join("blobs")).unwrap(),
            Queue::new(db),
            Sealer::ephemeral().unwrap(),
        );
        let origin = Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let control = Control::new(ports, origin);
        let account = Id::from_bytes([9; 16]);
        pollster::block_on(control.add_account(NewAccount { id: account, at: start() })).unwrap();
        let token = Token::from("a-token-only-the-owner-holds");
        let owner = Owner::new(account, &token);
        let api = Api::new(control, owner, (Manual::new(start(), 0), Counter::new()), limits);
        Self { api, token: token.reveal().to_owned(), account, _folder: folder }
    }

    fn send(&self, (method, path): (Method, &str), authorization: Option<&str>, body: Payload) -> Reply {
        let request = Request {
            method,
            path: path.to_owned(),
            authorization: authorization.map(|token| format!("Bearer {token}").into_bytes()),
            cookie: None,
            source: None,
            body,
        };
        pollster::block_on(self.api.respond(&request))
    }

    /// A call as the owner, with its JSON answer.
    fn call(&self, method: Method, path: &str, body: &[u8]) -> (Reply, Value) {
        let reply = self.send((method, path), Some(&self.token), Payload::Whole(body.to_vec()));
        let value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
        (reply, value)
    }

    /// Adds a site as the owner and returns its id.
    fn site(&self, handle: &str, source: &Value) -> String {
        let body = json!({ "handle": handle, "source": source }).to_string();
        let (reply, value) = self.call(Method::Post, "/v1/sites", body.as_bytes());
        assert_eq!(reply.status, Status::Created, "{value}");
        value["id"].as_str().unwrap().to_owned()
    }

    fn upload(&self, handle: &str) -> String {
        self.site(handle, &json!({ "kind": "upload" }))
    }
}

/// The archive of a built folder that serves `pages` (route, text), written as `ink build` does.
fn built(pages: &[(&str, &str)]) -> Vec<u8> {
    let entries: Vec<Served> = pages
        .iter()
        .map(|(route, text)| Served::new(Route::parse(*route).unwrap(), Hash::of(text), MediaType::Html, vec![]))
        .collect();
    let manifest = Manifest::new(entries).unwrap();
    let files: Vec<Value> = pages
        .iter()
        .enumerate()
        .map(|(n, (_, text))| json!({ "path": format!("page{n}.html"), "hash": Hash::of(text) }))
        .collect();
    let entries: Vec<Value> = manifest
        .entries()
        .iter()
        .map(|entry| {
            json!({
                "route": entry.route(), "blob": entry.blob(), "media": entry.media(),
                "status": entry.status(), "headers": [],
            })
        })
        .collect();
    let record = json!({
        "schema": "uses.manifest/v3", "deploy": manifest.id(), "files": files,
        "manifest": { "entries": entries },
    });
    let mut writer = Writer::new(1 << 20);
    writer.file(".ink-manifest.json", record.to_string().as_bytes()).unwrap();
    for (n, (_, text)) in pages.iter().enumerate() {
        writer.file(&format!("page{n}.html"), text.as_bytes()).unwrap();
    }
    writer.finish()
}

#[test]
fn every_call_needs_the_owner_token() {
    let fixture = Fixture::new(Limits::default());
    let none = fixture.send((Method::Get, "/v1/sites"), None, Payload::Whole(vec![]));
    assert_eq!((none.status, none.challenge), (Status::Unauthorized, true));
    let wrong = fixture.send((Method::Get, "/v1/sites"), Some("not the token"), Payload::Whole(vec![]));
    assert_eq!(wrong.status, Status::Unauthorized);
    let basic = Request {
        method: Method::Get,
        path: "/v1/sites".into(),
        authorization: Some(b"Basic abc".to_vec()),
        cookie: None,
        source: None,
        body: Payload::Whole(vec![]),
    };
    assert_eq!(pollster::block_on(fixture.api.respond(&basic)).status, Status::Unauthorized);
    // a path that is no route is not told to a caller with no token
    let hidden = fixture.send((Method::Get, "/nothing/here"), None, Payload::Whole(vec![]));
    assert_eq!(hidden.status, Status::Unauthorized);
    let (listed, value) = fixture.call(Method::Get, "/v1/sites", b"");
    assert_eq!((listed.status, value["items"].as_array().unwrap().len()), (Status::Ok, 0));
}

#[test]
fn a_site_is_added_listed_and_seen_with_its_address() {
    let fixture = Fixture::new(Limits::default());
    let id = fixture.upload("diary");
    let (_, listed) = fixture.call(Method::Get, "/v1/sites", b"");
    assert_eq!(listed["items"][0]["handle"], "diary");
    assert_eq!(listed["items"][0]["url"], "https://diary.uses.ink");
    let (reply, status) = fixture.call(Method::Get, &format!("/v1/sites/{id}"), b"");
    assert_eq!(reply.status, Status::Ok);
    assert_eq!((&status["live"], &status["build"]), (&Value::Null, &Value::Null));
    assert_eq!(status["site"]["source"]["kind"], "upload");
}

#[test]
fn a_bad_site_is_refused_with_its_reason() {
    let fixture = Fixture::new(Limits::default());
    fixture.upload("diary");
    let add = |body: Value| fixture.call(Method::Post, "/v1/sites", body.to_string().as_bytes());
    let (taken, problem) = add(json!({ "handle": "diary", "source": { "kind": "upload" } }));
    assert_eq!((taken.status, problem["problem"].as_str()), (Status::Conflict, Some("control")));
    let (reserved, _) = add(json!({ "handle": "www", "source": { "kind": "upload" } }));
    assert_eq!(reserved.status, Status::Conflict);
    let (bad, problem) = add(json!({ "handle": "-no-", "source": { "kind": "upload" } }));
    assert_eq!(bad.status, Status::BadRequest);
    assert!(!problem["causes"].as_array().unwrap().is_empty(), "{problem}");
    let (file, problem) =
        add(json!({ "handle": "local", "source": { "kind": "git", "url": "http://insecure.example/r" } }));
    assert_eq!((file.status, problem["problem"].as_str()), (Status::Unprocessable, Some("scheme")));
    let (garbage, _) = fixture.call(Method::Post, "/v1/sites", b"not json");
    assert_eq!(garbage.status, Status::BadRequest);
}

#[test]
fn a_built_folder_becomes_a_deploy_and_can_be_exported_and_rolled_back() {
    let fixture = Fixture::new(Limits::default());
    let id = fixture.upload("diary");
    let put = |pages: &[(&str, &str)]| fixture.call(Method::Put, &format!("/v1/sites/{id}/deploy"), &built(pages));
    let (first, one) = put(&[("/", "hello"), ("/about", "about")]);
    assert_eq!(
        (first.status, one["published"].as_str(), one["files"].as_u64()),
        (Status::Created, Some("live"), Some(3))
    );
    let (_, two) = put(&[("/", "hello again")]);
    assert_eq!(two["published"], "live");
    let (_, deploys) = fixture.call(Method::Get, &format!("/v1/sites/{id}/deploys"), b"");
    let items = deploys["items"].as_array().unwrap();
    assert_eq!((items.len(), items[0]["live"].clone(), items[1]["live"].clone()), (2, json!(true), json!(false)));
    // the same folder again is the same deploy, and changes nothing
    let (_, again) = put(&[("/", "hello again")]);
    assert_eq!((again["deploy"].clone(), again["published"].as_str()), (two["deploy"].clone(), Some("unchanged")));
    let (exported, _) = fixture.call(Method::Get, &format!("/v1/sites/{id}/export"), b"");
    assert_eq!(exported.status, Status::Ok);
    let tree = edge_api::tar::Tree::read(&exported.body, &Limits::default().archives()).unwrap();
    assert_eq!(tree.get(&model::ContentPath::parse("index").unwrap()), Some(b"hello again".as_slice()));
    let back = json!({ "deploy": one["deploy"] }).to_string();
    let (rolled, moved) = fixture.call(Method::Post, &format!("/v1/sites/{id}/rollback"), back.as_bytes());
    assert_eq!(
        (rolled.status, moved["live"]["paused"].clone(), moved["live"]["deploy"].clone()),
        (Status::Ok, json!(true), one["deploy"].clone())
    );
    let (twice, problem) = fixture.call(Method::Post, &format!("/v1/sites/{id}/rollback"), back.as_bytes());
    assert_eq!((twice.status, problem["problem"].as_str()), (Status::Conflict, Some("control")));
    let (_, held) = put(&[("/", "third")]);
    assert_eq!(held["published"], "held");
    let (resumed, moved) = fixture.call(Method::Post, &format!("/v1/sites/{id}/resume"), b"");
    assert_eq!((resumed.status, moved["was_paused"].clone()), (Status::Ok, json!(true)));
    let (_, status) = fixture.call(Method::Get, &format!("/v1/sites/{id}"), b"");
    assert_eq!(status["live"]["paused"], false);
}

#[test]
fn a_site_of_another_account_is_no_site() {
    let fixture = Fixture::new(Limits::default());
    let stranger: Id<AccountKind> = Id::from_bytes([3; 16]);
    pollster::block_on(fixture.api_control().add_account(NewAccount { id: stranger, at: start() })).unwrap();
    let theirs = pollster::block_on(fixture.api_control().add_site(control::NewSite {
        id: Id::from_bytes([4; 16]),
        account: stranger,
        handle: "theirs".parse().unwrap(),
        source: build::Source::Upload,
        at: start(),
    }))
    .unwrap();
    let (status, _) = fixture.call(Method::Get, &format!("/v1/sites/{}", theirs.id), b"");
    assert_eq!(status.status, Status::NotFound);
    let (removed, _) = fixture.call(Method::Delete, &format!("/v1/sites/{}", theirs.id), b"");
    assert_eq!(removed.status, Status::NotFound);
    let (listed, _) = fixture.call(Method::Get, "/v1/sites", b"");
    assert_eq!(listed.status, Status::Ok);
    assert_eq!(fixture.account, Id::from_bytes([9; 16]));
}

impl Fixture {
    fn api_control(&self) -> &Control<Ports> {
        self.api.control()
    }
}

#[test]
fn a_build_of_a_git_site_is_queued_and_of_an_upload_site_refused() {
    let fixture = Fixture::new(Limits::default());
    let git = fixture.site("journal", &json!({ "kind": "git", "url": "https://codeberg.org/a/b", "revision": "main" }));
    let (queued, value) = fixture.call(Method::Post, &format!("/v1/sites/{git}/builds"), b"");
    assert_eq!((queued.status, value["queued"]["build"].is_string()), (Status::Created, true));
    let (_, status) = fixture.call(Method::Get, &format!("/v1/sites/{git}"), b"");
    assert_eq!(
        (status["build"]["state"].clone(), status["build"]["trigger"].clone()),
        (json!("queued"), json!("manual"))
    );
    let build = status["build"]["id"].as_str().unwrap();
    let (log, problem) = fixture.call(Method::Get, &format!("/v1/sites/{git}/builds/{build}/log"), b"");
    assert_eq!((log.status, problem["problem"].as_str()), (Status::NotFound, Some("no-log")));
    let upload = fixture.upload("diary");
    let (refused, problem) = fixture.call(Method::Post, &format!("/v1/sites/{upload}/builds"), b"");
    assert_eq!((refused.status, problem["problem"].as_str()), (Status::Unprocessable, Some("unbuilt")));
}

#[test]
fn removing_a_site_says_what_went() {
    let fixture = Fixture::new(Limits::default());
    let id = fixture.upload("diary");
    fixture.call(Method::Put, &format!("/v1/sites/{id}/deploy"), &built(&[("/", "hello")]));
    let (removed, receipt) = fixture.call(Method::Delete, &format!("/v1/sites/{id}"), b"");
    assert_eq!(
        (removed.status, receipt["deploys"].clone(), receipt["blobs"].clone()),
        (Status::Ok, json!(1), json!(2))
    );
    let (gone, _) = fixture.call(Method::Get, &format!("/v1/sites/{id}"), b"");
    assert_eq!(gone.status, Status::NotFound);
    let (again, _) = fixture.call(
        Method::Post,
        "/v1/sites",
        json!({ "handle": "diary", "source": { "kind": "upload" } }).to_string().as_bytes(),
    );
    assert_eq!(again.status, Status::Conflict, "a removed handle stays taken");
}

#[test]
fn routes_say_what_they_take() {
    let fixture = Fixture::new(Limits::default());
    let (wrong, _) = fixture.call(Method::Delete, "/v1/sites", b"");
    assert_eq!((wrong.status, wrong.allow), (Status::MethodNotAllowed, vec![Method::Get, Method::Post]));
    let (missing, _) = fixture.call(Method::Get, "/v1/nothing", b"");
    assert_eq!(missing.status, Status::NotFound);
    let (bad, problem) = fixture.call(Method::Get, "/v1/sites/not-an-id", b"");
    assert_eq!((bad.status, problem["problem"].as_str()), (Status::BadRequest, Some("route")));
    let (over, problem) = {
        let reply = fixture.send((Method::Post, "/v1/sites"), Some(&fixture.token), Payload::Over { limit: 65536 });
        let value = serde_json::from_slice::<Value>(&reply.body).unwrap();
        (reply, value)
    };
    assert_eq!((over.status, problem["problem"].as_str()), (Status::TooLarge, Some("too-large")));
    assert_eq!(
        fixture.api.limits().budget(Method::Put, &format!("/v1/sites/{}/deploy", "00".repeat(16))),
        Limits::default().archive
    );
    assert_eq!(fixture.api.limits().budget(Method::Post, "/v1/sites"), Limits::default().body);
}
