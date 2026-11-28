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

mod support;

use control::{AccountKind, Control, Id, NewAccount};
use edge_api::tar::Writer;
use edge_api::{Limits, Method, Payload, Status};
use model::{Hash, Manifest, MediaType, Route, Served};
use serde_json::{Value, json};
use support::{Ports, Rig, signer, start};

/// The rig, as the operator calls it.
struct Fixture(Rig);

impl std::ops::Deref for Fixture {
    type Target = Rig;

    fn deref(&self) -> &Rig {
        &self.0
    }
}

impl Fixture {
    fn new(limits: Limits) -> Self {
        Self(Rig::new(limits))
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
fn every_call_needs_a_signature() {
    let fixture = Fixture::new(Limits::default());
    let none = fixture.answer(&Rig::unsigned((Method::Get, "/v1/sites"), None, b""));
    assert_eq!((none.status, none.challenge), (Status::BadRequest, true));
    let basic = fixture.answer(&Rig::unsigned((Method::Get, "/v1/sites"), Some(b"Bearer abc".to_vec()), b""));
    assert_eq!(basic.status, Status::BadRequest);
    // a path that is no route is not told to a caller with no signature
    let hidden = fixture.answer(&Rig::unsigned((Method::Get, "/nothing/here"), None, b""));
    assert_eq!(hidden.status, Status::BadRequest);
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
    assert_eq!((taken.status, problem["problem"].as_str()), (Status::Conflict, Some("handle.name.taken")));
    let (reserved, _) = add(json!({ "handle": "www", "source": { "kind": "upload" } }));
    assert_eq!(reserved.status, Status::Conflict);
    let (bad, problem) = add(json!({ "handle": "-no-", "source": { "kind": "upload" } }));
    assert_eq!(bad.status, Status::BadRequest);
    assert!(!problem["causes"].as_array().unwrap().is_empty(), "{problem}");
    let (file, problem) = add(json!({ "handle": "local", "source": { "kind": "git", "url": "file:///srv/repo" } }));
    assert_eq!((file.status, problem["problem"].as_str()), (Status::Unprocessable, Some("scheme")));
    let (insecure, _) =
        add(json!({ "handle": "plain", "source": { "kind": "git", "url": "http://insecure.example/r" } }));
    assert_eq!(insecure.status, Status::BadRequest, "an address that is no repository locator is no body");
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
    let (doubled, problem) = fixture.call(Method::Post, &format!("/v1/sites/{id}/rollback"), back.as_bytes());
    assert_eq!((doubled.status, problem["problem"].as_str()), (Status::Conflict, Some("deploy.state.live")));
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
        let mut request = fixture.request(&fixture.operator, (Method::Post, "/v1/sites"), b"");
        request.body = Payload::Over { limit: 65536 };
        let reply = fixture.answer(&request);
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

/// A report of `handle` as a stranger at `source` sends it.
fn reported(fixture: &Fixture, source: &str, handle: &str) -> (edge_api::Reply, Value) {
    let body = json!({ "handle": handle, "kind": "phishing", "detail": "asks for a bank login" });
    let mut request = Rig::unsigned((Method::Post, "/v1/abuse"), None, body.to_string().as_bytes());
    request.source = Some(source.parse().unwrap());
    let reply = fixture.answer(&request);
    let value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
    (reply, value)
}

#[test]
fn abuse_reports_are_taken_unsigned_and_limited_per_source() {
    let fixture = Fixture::new(Limits::default());
    for _ in 0..5 {
        let (reply, value) = reported(&fixture, "203.0.113.7", "theirs");
        assert_eq!(reply.status, Status::Created, "{value}");
        assert_eq!(value["id"].as_str().map(str::len), Some(32));
    }
    let (limited, problem) = reported(&fixture, "203.0.113.7", "theirs");
    assert_eq!((limited.status, problem["problem"].as_str()), (Status::TooMany, Some("abuse.report.limited")));
    assert!(limited.retry.is_some() && problem["fields"]["seconds"].is_string(), "{problem}");
    // another source has its own count, and the same /64 shares one
    assert_eq!(reported(&fixture, "203.0.113.8", "theirs").0.status, Status::Created);
    // a report that does not read is refused whole, and a refused one is still counted
    let long = json!({ "handle": "theirs", "kind": "spam", "detail": "x".repeat(3000) }).to_string();
    let mut request = Rig::unsigned((Method::Post, "/v1/abuse"), None, long.as_bytes());
    request.source = Some("198.51.100.1".parse().unwrap());
    assert_eq!(fixture.answer(&request).status, Status::BadRequest);
}

#[test]
fn only_the_operator_reads_reports_and_suspends() {
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
    let (_, filed) = reported(&fixture, "203.0.113.7", "theirs");
    let device = support::signer(5);
    fixture.enrolled(&device, &json!({}));
    for (method, path) in [
        (Method::Get, "/v1/abuse".to_owned()),
        (Method::Post, format!("/v1/sites/{}/suspend", theirs.id)),
        (Method::Post, format!("/v1/sites/{}/reinstate", theirs.id)),
    ] {
        let (denied, problem) = fixture.call_as(&device, method, &path, br#"{"kind":"spam"}"#);
        assert_eq!(
            (denied.status, problem["problem"].as_str()),
            (Status::Forbidden, Some("auth.operator.required")),
            "{path}"
        );
    }
    let (listed, value) = fixture.call(Method::Get, "/v1/abuse", b"");
    assert_eq!((listed.status, value["items"][0]["state"].as_str()), (Status::Ok, Some("open")));
    let body = json!({ "kind": "phishing", "report": filed["id"] }).to_string();
    let path = format!("/v1/sites/{}/suspend", theirs.id);
    let (done, value) = fixture.call(Method::Post, &path, body.as_bytes());
    assert_eq!((done.status, value["kind"].as_str()), (Status::Ok, Some("phishing")));
    let (_, value) = fixture.call(Method::Get, "/v1/abuse", b"");
    assert_eq!(value["items"][0]["state"], "actioned");
    let (repeat, problem) = fixture.call(Method::Post, &path, body.as_bytes());
    assert_eq!((repeat.status, problem["problem"].as_str()), (Status::Forbidden, Some("site.state.suspended")));
    let (restored, _) = fixture.call(Method::Post, &format!("/v1/sites/{}/reinstate", theirs.id), b"");
    assert_eq!(restored.status, Status::Ok);
    let (unlifted, problem) = fixture.call(Method::Post, &format!("/v1/sites/{}/reinstate", theirs.id), b"");
    assert_eq!((unlifted.status, problem["problem"].as_str()), (Status::Conflict, Some("site.state.serving")));
}

#[test]
fn a_branch_preview_is_pushed_listed_and_never_a_deploy() {
    let fixture = Fixture::new(Limits::default());
    let git = fixture.site("diary", &json!({ "kind": "git", "url": "https://codeberg.org/cstef/diary" }));
    let upload = fixture.upload("folder");
    let archive = built(&[("/", "<h1>branch</h1>")]);
    let path = format!("/v1/sites/{git}/previews/new-design");
    let (made, value) = fixture.call(Method::Put, &path, &archive);
    assert_eq!(made.status, Status::Created, "{value}");
    assert_eq!(value["slug"], "new-design");
    assert_eq!(value["url"], "https://new-design--diary.uses.ink");
    let (listed, value) = fixture.call(Method::Get, &format!("/v1/sites/{git}/previews"), b"");
    assert_eq!((listed.status, value["items"].as_array().map(Vec::len)), (Status::Ok, Some(1)));
    // it is no deploy: the site has none, and nothing is live
    let (_, deploys) = fixture.call(Method::Get, &format!("/v1/sites/{git}/deploys"), b"");
    assert_eq!(deploys["items"].as_array().map(Vec::len), Some(0));
    let (_, status) = fixture.call(Method::Get, &format!("/v1/sites/{git}"), b"");
    assert_eq!(status["live"], Value::Null);
    // an upload site has no branches, and a label that is no label is no route
    let (refused, problem) = fixture.call(Method::Put, &format!("/v1/sites/{upload}/previews/main"), &archive);
    assert_eq!(
        (refused.status, problem["problem"].as_str()),
        (Status::Unprocessable, Some("preview.site.unsupported"))
    );
    let (bad, _) = fixture.call(Method::Put, &format!("/v1/sites/{git}/previews/a--b"), &archive);
    assert_eq!(bad.status, Status::BadRequest);
    // a preview lasts seven days, and then it is not listed
    fixture.clock.advance(std::time::Duration::from_secs(7 * 24 * 3600 + 1));
    let (_, gone) = fixture.call(Method::Get, &format!("/v1/sites/{git}/previews"), b"");
    assert_eq!(gone["items"].as_array().map(Vec::len), Some(0));
}

#[test]
fn a_custom_domain_is_claimed_listed_and_removed_and_its_proof_needs_a_resolver() {
    let fixture = Fixture::new(Limits::default());
    let site = fixture.upload("tea");
    let path = format!("/v1/sites/{site}/domains/www.example.com");
    let (made, value) = fixture.call(Method::Put, &path, b"");
    assert_eq!(made.status, Status::Created, "{value}");
    assert_eq!(
        (value["state"].as_str(), value["record"]["host"].as_str()),
        (Some("pending"), Some("_uses-ink.www.example.com"))
    );
    assert!(value["record"]["value"].as_str().unwrap().starts_with("uses-ink-domain="));
    let (listed, value) = fixture.call(Method::Get, &format!("/v1/sites/{site}/domains"), b"");
    assert_eq!((listed.status, value["items"].as_array().map(Vec::len)), (Status::Ok, Some(1)));
    // this server has no DNS resolver: the proof says so and nothing is proved
    let (unchecked, problem) = fixture.call(Method::Post, &format!("{path}/proof"), b"");
    assert_eq!((unchecked.status, problem["problem"].as_str()), (Status::Unavailable, Some("domain.lookup.failed")));
    assert!(problem["causes"][1].as_str().unwrap().contains("no DNS resolver"), "{problem}");
    // a name uses.ink serves itself, and text that is no name
    let (ours, problem) = fixture.call(Method::Put, &format!("/v1/sites/{site}/domains/tea.uses.ink"), b"");
    assert_eq!((ours.status, problem["problem"].as_str()), (Status::Unprocessable, Some("domain.name.ours")));
    let (bad, _) = fixture.call(Method::Put, &format!("/v1/sites/{site}/domains/localhost"), b"");
    assert_eq!(bad.status, Status::BadRequest);
    // the record is read first, so a name nobody claimed answers the same without a resolver
    let (none, problem) = fixture.call(Method::Post, &format!("/v1/sites/{site}/domains/none.example.com/proof"), b"");
    assert_eq!((none.status, problem["problem"].as_str()), (Status::Unavailable, Some("domain.lookup.failed")));
    let (removed, value) = fixture.call(Method::Delete, &path, b"");
    assert_eq!((removed.status, value["name"].as_str()), (Status::Ok, Some("www.example.com")));
    let (again, problem) = fixture.call(Method::Delete, &path, b"");
    assert_eq!((again.status, problem["problem"].as_str()), (Status::NotFound, Some("domain.name.unknown")));
}

impl Fixture {
    /// A second person, with an account and no site.
    fn person(&self, byte: u8) -> Id<AccountKind> {
        let id = Id::from_bytes([byte; 16]);
        pollster::block_on(self.api_control().add_account(NewAccount { id, at: start() })).unwrap();
        id
    }

    /// An organization made by the operator.
    fn org(&self, name: &str) -> String {
        let (reply, org) = self.call(Method::Post, "/v1/orgs", json!({ "name": name }).to_string().as_bytes());
        assert_eq!(reply.status, Status::Created, "{org}");
        org["id"].as_str().unwrap().to_owned()
    }
}

#[test]
fn an_organization_is_made_listed_and_has_its_maker_as_owner() {
    let fixture = Fixture::new(Limits::default());
    let org = fixture.org("Garden club");
    let (listed, orgs) = fixture.call(Method::Get, "/v1/orgs", b"");
    assert_eq!(listed.status, Status::Ok);
    assert_eq!(
        (orgs["items"][0]["name"].as_str(), orgs["items"][0]["role"].as_str()),
        (Some("Garden club"), Some("owner"))
    );
    let (_, shown) = fixture.call(Method::Get, &format!("/v1/orgs/{org}"), b"");
    assert_eq!(shown["members"].as_array().unwrap().len(), 1);
    assert_eq!(shown["members"][0]["role"], "owner");
    let (bad, problem) = fixture.call(Method::Post, "/v1/orgs", br#"{"name":""}"#);
    assert_eq!(bad.status, Status::BadRequest, "{problem}");
    let unknown = format!("/v1/orgs/{}", "ab".repeat(16));
    assert_eq!(fixture.call(Method::Get, &unknown, b"").0.status, Status::NotFound);
}

#[test]
fn members_are_added_regraded_and_removed_and_the_last_owner_stays() {
    let fixture = Fixture::new(Limits::default());
    let org = fixture.org("Garden club");
    let friend = fixture.person(5);
    let members = format!("/v1/orgs/{org}/members");
    let add = json!({ "account": friend.to_string(), "role": "writer" }).to_string();
    let (added, member) = fixture.call(Method::Post, &members, add.as_bytes());
    assert_eq!((added.status, member["role"].as_str()), (Status::Created, Some("writer")), "{member}");
    let (again, problem) = fixture.call(Method::Post, &members, add.as_bytes());
    assert_eq!((again.status, problem["problem"].as_str()), (Status::Conflict, Some("org.member.exists")));
    let one = format!("{members}/{friend}");
    let (set, member) = fixture.call(Method::Put, &one, br#"{"role":"admin"}"#);
    assert_eq!((set.status, member["role"].as_str()), (Status::Ok, Some("admin")));
    let (bad, _) = fixture.call(Method::Put, &one, br#"{"role":"boss"}"#);
    assert_eq!(bad.status, Status::BadRequest);

    let me = format!("{members}/{}", fixture.account);
    let (last, problem) = fixture.call(Method::Delete, &me, b"");
    assert_eq!((last.status, problem["problem"].as_str()), (Status::Conflict, Some("org.owner.last")), "{problem}");
    let (demoted, problem) = fixture.call(Method::Put, &me, br#"{"role":"admin"}"#);
    assert_eq!((demoted.status, problem["problem"].as_str()), (Status::Conflict, Some("org.owner.last")));

    assert_eq!(fixture.call(Method::Delete, &one, b"").0.status, Status::Ok);
    let (_, shown) = fixture.call(Method::Get, &format!("/v1/orgs/{org}"), b"");
    assert_eq!(shown["members"].as_array().unwrap().len(), 1);
}

#[test]
fn a_site_made_as_an_organization_belongs_to_it_and_not_to_the_member() {
    let fixture = Fixture::new(Limits::default());
    let org = fixture.org("Garden club");
    let body = json!({ "handle": "club", "source": { "kind": "upload" } }).to_string();
    let (made, site) = fixture.call(Method::Post, &format!("/v1/orgs/{org}/sites"), body.as_bytes());
    assert_eq!(made.status, Status::Created, "{site}");
    let id = site["id"].as_str().unwrap().to_owned();
    let (mine, mine_items) = fixture.call(Method::Get, "/v1/sites", b"");
    assert_eq!((mine.status, mine_items["items"].as_array().unwrap().len()), (Status::Ok, 0));
    let (theirs, items) = fixture.call(Method::Get, &format!("/v1/orgs/{org}/sites"), b"");
    assert_eq!((theirs.status, items["items"][0]["handle"].as_str()), (Status::Ok, Some("club")));
    // the member's own path does not reach it, and a stranger finds no organization
    assert_eq!(fixture.call(Method::Get, &format!("/v1/sites/{id}"), b"").0.status, Status::NotFound);
    let other = fixture.org("Other");
    let (foreign, _) = fixture.call(Method::Get, &format!("/v1/orgs/{other}/sites/{id}"), b"");
    assert_eq!(foreign.status, Status::NotFound);
    let nobody = format!("/v1/orgs/{}/sites", "cd".repeat(16));
    let (none, problem) = fixture.call(Method::Get, &nobody, b"");
    assert_eq!((none.status, problem["problem"].as_str()), (Status::NotFound, Some("org.id.unknown")), "{problem}");
}

#[test]
fn a_device_key_without_the_members_right_cannot_change_members() {
    let fixture = Fixture::new(Limits::default());
    let org = fixture.org("Garden club");
    let friend = fixture.person(5);
    let device = signer(7);
    let (_, code) = fixture.ask(&device, &json!({}));
    let approval = control::keys::Approval {
        code: code.parse().unwrap(),
        fingerprint: device.id().fingerprint(),
        by: fixture.account,
        within: control::keys::Rights::DEVICE,
    };
    pollster::block_on(fixture.enrol.keys().approve(approval, fixture.clock_now())).unwrap();
    let add = json!({ "account": friend.to_string(), "role": "reader" }).to_string();
    let (denied, problem) = fixture.call_as(&device, Method::Post, &format!("/v1/orgs/{org}/members"), add.as_bytes());
    assert_eq!(denied.status, Status::Forbidden, "{problem}");
    // the same key reads the organization's sites, because it reaches it with Read
    let (read, _) = fixture.call_as(&device, Method::Get, &format!("/v1/orgs/{org}/sites"), b"");
    assert_eq!(read.status, Status::Ok);
}
