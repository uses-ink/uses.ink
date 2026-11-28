//! Organizations through the real binary (decision 0095): `ink org create|list|member`, acting as
//! an organization with `--org`, an org deploy key that outlives the member who approved it, and
//! the Account screen's forms.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

mod keyed;
mod rig;

use std::time::{Duration, Instant};

use rig::{Browser, Machine, Server};
use serde_json::json;

/// The path of a link the server printed: what a browser would request.
fn target(link: &str) -> String {
    format!("/v1{}", &link[link.find("/login/").unwrap()..])
}

/// A machine holding the operator's key: the account that owns the server.
fn operator(server: &Server) -> Machine<'_> {
    let mut machine = server.machine("operator");
    machine.dir = server.root.join("site");
    std::fs::create_dir_all(&machine.dir).unwrap();
    let logged = machine.ink(&["login", "--data", server.data.to_str().unwrap(), "--label", "operator"]);
    assert_eq!(logged.code, 0, "{}", logged.said());
    machine
}

/// A person who signs up by the mail link the server prints, and their account id. `mails` is how
/// many sign-in links the server has printed once theirs is among them.
fn person<'a>(server: &'a Server, address: &str, mails: usize) -> (Browser<'a>, String) {
    let mut browser = server.browser();
    let asked = browser.send(("POST", "/v1/login"), &json!({ "address": address }).to_string());
    assert_eq!(asked.status(), "202", "{}", asked.text);
    let started = Instant::now();
    while server.links().len() < mails {
        assert!(started.elapsed() < Server::PATIENCE, "no mail came: {}", server.said());
        std::thread::sleep(Duration::from_millis(100));
    }
    let link = server.links().pop().unwrap();
    let entered = browser.send(("POST", &target(&link)), "");
    assert_eq!(entered.status(), "200", "{}{}", entered.text, server.said());
    browser.keep(&entered);
    let account = browser.send(("GET", "/v1/session"), "").json()["account"].as_str().unwrap().to_owned();
    (browser, account)
}

#[test]
fn an_organization_is_made_staffed_and_acted_as_and_its_deploy_key_outlives_its_approver() {
    let server = Server::start("orgs-cli", &["--signup", "open", "--site-ttl", "0"]);
    let machine = operator(&server);
    let (ann, ann_id) = person(&server, "ann@example.org", 2);

    let made = machine.ink(&["org", "create", "Garden club", "--json"]);
    assert_eq!(made.code, 0, "{}", made.said());
    assert_eq!((made.json()["name"].as_str(), made.json()["role"].as_str()), (Some("Garden club"), Some("owner")));
    let org = made.json()["id"].as_str().unwrap().to_owned();
    let listed = machine.ink(&["org", "list"]);
    assert!(listed.out.contains("Garden club") && listed.out.contains("owner"), "{}", listed.said());
    let bad = machine.ink(&["org", "create", ""]);
    assert_eq!(bad.code, 2, "{}", bad.said());

    // staffing: add, regrade, and the last owner is kept
    let added = machine.ink(&["org", "member", "add", &org, &ann_id, "--role", "writer", "--json"]);
    assert_eq!(added.code, 0, "{}", added.said());
    assert_eq!(added.json()["role"], "writer");
    let again = machine.ink(&["org", "member", "add", &org, &ann_id]);
    assert_eq!(again.code, 5, "{}", again.said());
    assert!(again.err.contains("org.member.exists"), "{}", again.err);
    let members = machine.ink(&["org", "member", "list", &org, "--json"]);
    assert_eq!(members.code, 0, "{}", members.said());
    let rows = members.json()["members"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 2, "{}", members.out);
    let owner_id = rows.iter().find(|row| row["role"] == "owner").unwrap()["account"].as_str().unwrap().to_owned();
    let last = machine.ink(&["org", "member", "remove", &org, &owner_id]);
    assert_eq!(last.code, 5, "{}", last.said());
    assert!(last.err.contains("org.owner.last"), "{}", last.err);
    let regraded = machine.ink(&["org", "member", "role", &org, &ann_id, "admin", "--json"]);
    assert_eq!(regraded.json()["role"], "admin", "{}", regraded.said());
    machine.ink(&["org", "member", "role", &org, &ann_id, "writer"]);

    // acting as the organization: the site is its, not the member's
    machine.write("index.md", "# Club\n\nThe club's page.\n");
    let init = machine.ink(&["init", "--org", &org, "--handle", "club", "--json"]);
    assert_eq!(init.code, 0, "{}", init.said());
    let site = init.json()["site"].as_str().unwrap().to_owned();
    let pushed = machine.ink(&["push", "--org", &org, "--wait", "30"]);
    assert_eq!(pushed.code, 0, "{}", pushed.said());
    assert!(server.page("club", "/").body().contains("The club's page."));
    let hers = ann.send(("GET", "/account"), "");
    assert_eq!(hers.status(), "200", "{}", hers.text);
    assert!(!hers.body().contains("href=\"/club\""), "Ann has no site of her own: {}", hers.body());

    // Ann approves a deploy key for the organization; removing her leaves the key working
    let mut ci = server.machine("ci");
    ci.dir = machine.dir.clone();
    let key = ci.home.join("ci.key");
    let key = key.to_str().unwrap().to_owned();
    let waiting =
        ci.start(&["key", "add", "--site", &site, "--label", "ci", "--out", &key, "--poll", "1", "--wait", "30"]);
    let fingerprint = waiting.until("fingerprint").split_whitespace().last().unwrap().to_owned();
    let asked = waiting.until("/v1/approve/");
    let code = asked.split("/v1/approve/").nth(1).unwrap().split(',').next().unwrap().to_owned();
    let path = format!("/v1/approve/{code}");
    let body = json!({ "fingerprint": fingerprint, "org": org }).to_string();
    let approved = ann.send(("POST", &path), &body);
    assert_eq!(approved.status(), "200", "{}{}", approved.text, server.said());
    let (done, said) = waiting.finish();
    assert_eq!(done, 0, "{said}");
    ci.env.push(("INK_KEY_FILE".to_owned(), key));

    let operator_key = keyed::Keyed::operator(&server.data, &server.audience);
    let keys_path = format!("/v1/orgs/{org}/keys");
    let read_keys = || {
        let auth = operator_key.sign("GET", &keys_path, b"");
        let headers = [("Authorization", auth.as_str())];
        server.once(("GET", &keys_path), &headers, "")
    };
    let keys = read_keys();
    assert_eq!(keys.status(), "200", "{}", keys.text);
    let before = keys.json()["items"].as_array().unwrap().clone();
    assert_eq!(before.len(), 1, "{}", keys.text);
    assert_eq!(before[0]["creator_removed"], false);
    let out = machine.ink(&["org", "member", "remove", &org, &ann_id, "--json"]);
    assert_eq!(out.code, 0, "{}", out.said());
    machine.write("index.md", "# Club\n\nAfter Ann left.\n");
    let queued = ci.ink(&["push", "--no-wait"]);
    assert_eq!(queued.code, 0, "the org's key outlives the member who approved it: {}", queued.said());
    let started = Instant::now();
    while !server.page("club", "/").body().contains("After Ann left.") {
        assert!(started.elapsed() < Server::PATIENCE, "the queued build never went live: {}", server.said());
        std::thread::sleep(Duration::from_millis(100));
    }
    let after = read_keys().json()["items"].as_array().unwrap().clone();
    assert_eq!(after[0]["creator_removed"], true, "the list says the approver is gone: {after:?}");
    // Ann, no longer a member, finds no organization
    let gone = ann.send(("GET", "/account"), "");
    assert!(!gone.body().contains("Garden club"), "{}", gone.body());
}

#[test]
fn the_account_screen_makes_an_organization_and_changes_who_is_in_it() {
    let server = Server::start("orgs-web", &["--signup", "open", "--site-ttl", "0"]);
    let link = server.links().pop().unwrap();
    let path = link[link.find("/login/").unwrap()..].to_owned();
    let mut owner = server.browser();
    let entered = owner.send(("POST", &path), "");
    assert_eq!(entered.status(), "303", "{}", entered.text);
    owner.keep(&entered);
    let (_ann, ann_id) = person(&server, "ann@example.org", 2);

    let empty = owner.send(("GET", "/account"), "");
    assert_eq!(empty.status(), "200", "{}", empty.text);
    assert!(empty.body().contains("action=\"/account/orgs\""), "the make form: {}", empty.body());

    let bad = owner.send(("POST", "/account/orgs"), "name=");
    assert_eq!(bad.status(), "400", "{}", bad.text);
    let foreign = owner.send_with(("POST", "/account/orgs"), &[("Origin", "http://evil.example")], "name=Club");
    assert_eq!(foreign.status(), "403", "a post from another origin changes nothing: {}", foreign.text);
    let made = owner.send(("POST", "/account/orgs"), "name=Garden+club");
    assert_eq!(made.status(), "303", "{}", made.text);
    assert_eq!(made.header("location"), Some("/account"));

    let shown = owner.send(("GET", "/account"), "");
    let body = shown.body();
    assert!(body.contains("Garden club") && body.contains("action=\"/account/members\""), "{body}");
    let at = body.find("Id <code").or_else(|| body.find("Id ")).unwrap();
    let org: String = body[at..].split("<code").nth(1).unwrap().split('>').nth(1).unwrap().chars().take(32).collect();
    assert_eq!(org.len(), 32, "{org} in {body}");

    let add = format!("org={org}&account={ann_id}&role=writer");
    let added = owner.send(("POST", "/account/members"), &add);
    assert_eq!(added.status(), "303", "{}{}", added.text, server.said());
    assert!(owner.send(("GET", "/account"), "").body().contains(&ann_id));
    let twice = owner.send(("POST", "/account/members"), &add);
    assert_eq!(twice.status(), "409", "{}", twice.text);
    assert!(twice.body().contains("a member of") && twice.body().contains("already"), "{}", twice.body());
    let regrade = owner.send(("POST", "/account/roles"), &format!("org={org}&account={ann_id}&role=admin"));
    assert_eq!(regrade.status(), "303", "{}", regrade.text);
    assert!(owner.send(("GET", "/account"), "").body().contains("admin"));
    let unknown = owner.send(("POST", "/account/roles"), &format!("org={org}&account={ann_id}&role=boss"));
    assert_eq!(unknown.status(), "400", "{}", unknown.text);
    let removed = owner.send(("POST", "/account/removals"), &format!("org={org}&account={ann_id}"));
    assert_eq!(removed.status(), "303", "{}", removed.text);
    let after = owner.send(("GET", "/account"), "");
    assert!(after.body().contains("1 members") && !after.body().contains(&ann_id), "{}", after.body());
}
