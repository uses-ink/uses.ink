//! A stranger's first hour on a self-hosted instance, through the real binary and no shortcut:
//! `ink serve --data` prints the first-run link once, the link signs a browser in, `ink login`
//! asks for a key that the browser approves by its code and fingerprint, and that key publishes a
//! folder, is served on its handle, makes a deploy key for CI, revokes it and logs out. A second
//! person signs in by the email link, which the capture mailer prints.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

mod rig;

use rig::{Machine, Server};
use serde_json::json;

/// The path of a link the server printed: what a browser would request.
fn target(link: &str) -> &str {
    &link[link.find("/v1/login/").unwrap()..]
}

/// Starts `ink <args>` on `machine`, approves the key it asks for in `browser` and returns what
/// the command printed.
fn approved(machine: &Machine<'_>, browser: &rig::Browser<'_>, args: &[&str]) -> (i32, String) {
    let waiting = machine.start(args);
    let fingerprint = waiting.until("fingerprint");
    let fingerprint = fingerprint.split_whitespace().last().unwrap().to_owned();
    let asked = waiting.until("/v1/approve/");
    let code = asked.split("/v1/approve/").nth(1).unwrap().split(',').next().unwrap().to_owned();
    let path = format!("/v1/approve/{code}");

    let seen = browser.send(("GET", &path), "");
    assert_eq!(seen.status(), "200", "{}", seen.text);
    assert_eq!(seen.json()["fingerprint"], fingerprint, "the screens show the same fingerprint");
    let wrong = browser.send(("POST", &path), &json!({ "fingerprint": "AAAA-AAAA-AA" }).to_string());
    assert_eq!(wrong.status(), "409", "approving a key whose fingerprint was not compared: {}", wrong.text);
    let right = browser.send(("POST", &path), &json!({ "fingerprint": fingerprint }).to_string());
    assert_eq!(right.status(), "200", "{}", right.text);
    waiting.finish()
}

#[test]
fn a_stranger_signs_in_enrols_a_machine_publishes_and_logs_out() {
    let server = Server::start("stranger", &["--signup", "open", "--site-ttl", "0"]);

    // the first-run link is printed once, and is the only way in until someone is in
    let links = server.links();
    assert_eq!(links.len(), 1, "one first-run link: {}", server.said());
    assert!(server.said().contains("shown once"), "the start says the link is shown once");
    assert!(
        server.said().contains(&format!("{}/v1/sites", server.api)),
        "the start gives the API address a client types, port included: {}",
        server.said()
    );
    let first = target(&links[0]).to_owned();
    let mut owner = server.browser();
    assert_eq!(owner.send(("GET", "/v1/session"), "").status(), "401", "no session before the link");
    let looked = server.http(("GET", &first), &[], "");
    assert_eq!(looked.status(), "200", "looking at a link is allowed: {}", looked.text);
    assert!(looked.sets_none(), "a GET only looks and signs nobody in");
    let entered = owner.send(("POST", &first), "");
    assert_eq!(entered.status(), "200", "{}", entered.text);
    owner.keep(&entered);
    let cookie = entered.set_cookie();
    assert!(cookie.starts_with("ink-session="), "{cookie}");
    for attribute in ["HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(cookie.contains(attribute), "{attribute} in {cookie}");
    }
    assert_eq!(owner.send(("GET", "/v1/session"), "").status(), "200");
    let spent = owner.send(("POST", &first), "");
    assert!(spent.status().starts_with('4'), "a link works once: {}", spent.text);

    // a machine asks for a key; the browser compares the fingerprint and approves it
    let mut laptop = server.machine("laptop");
    laptop.dir = server.root.join("site");
    std::fs::create_dir_all(&laptop.dir).unwrap();
    let nobody = laptop.ink(&["whoami"]);
    assert_eq!(nobody.code, 3, "no key yet: {}", nobody.said());
    let (code, said) = approved(&laptop, &owner, &["login", "--label", "laptop", "--poll", "1", "--wait", "30"]);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("enrolled"), "{said}");
    assert!(!said.contains("ink-ed25519-1:"), "the private key was printed: {said}");

    let who = laptop.ink(&["whoami", "--json"]);
    assert_eq!(who.code, 0, "{}", who.said());
    assert_eq!(who.json()["schema"], "uses.whoami/v1");
    assert_eq!(who.json()["label"], "laptop");
    let keys = laptop.ink(&["key", "list", "--json"]);
    assert_eq!(keys.code, 0, "{}", keys.said());
    assert_eq!(keys.json()["keys"].as_array().unwrap().len(), 1, "{}", keys.out);

    // init and push a folder; the build runs and the site is served on its handle
    laptop.write("index.md", "# Garden\n\nThe first version.\n");
    let init = laptop.ink(&["init", "--handle", "garden", "--json"]);
    assert_eq!(init.code, 0, "{}", init.said());
    let site = init.json()["site"].as_str().unwrap().to_owned();
    let pushed = laptop.ink(&["push", "--wait", "30", "--json"]);
    assert_eq!(pushed.code, 0, "{}{}", pushed.said(), server.said());
    assert_eq!(pushed.json()["live"], true);
    let page = server.page("garden", "/");
    assert_eq!(page.status(), "200", "{}", page.text);
    assert!(page.body().contains("The first version."), "{}", page.text);

    laptop.write("index.md", "# Garden\n\nThe second version.\n");
    let again = laptop.ink(&["push", "--wait", "30"]);
    assert_eq!(again.code, 0, "{}", again.said());
    assert!(server.page("garden", "/").body().contains("The second version."));
    let status = laptop.ink(&["status", "--json"]);
    assert_eq!(status.code, 0, "{}", status.said());
    assert_eq!(status.json()["health"], "ok");

    // a deploy key for one site: made by the laptop, approved by the person, held by CI alone
    let keyfile = server.root.join("ci.key");
    let key = keyfile.to_str().unwrap();
    let (code, said) = approved(
        &laptop,
        &owner,
        &["key", "add", "--site", &site, "--label", "ci", "--out", key, "--poll", "1", "--wait", "30"],
    );
    assert_eq!(code, 0, "{said}");
    let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&keyfile).unwrap().permissions());
    assert_eq!(mode & 0o077, 0, "the private key is readable by its owner alone");
    let mut ci = server.machine("ci");
    ci.dir = laptop.dir.clone();
    ci.env.push(("INK_KEY_FILE".to_owned(), key.to_owned()));
    laptop.write("index.md", "# Garden\n\nPublished by CI.\n");
    // a deploy key queues the build and may not read it: following it is refused, and says that
    // the build is the server's now and how to push without following
    let followed = ci.ink(&["push", "--wait", "30"]);
    assert_eq!(followed.code, 3, "{}", followed.said());
    assert!(followed.err.contains("auth.right.missing"), "{}", followed.err);
    assert!(
        followed.err.contains("is queued on the site"),
        "a refusal after queueing is not `nothing changed`: {}",
        followed.err
    );
    assert!(followed.err.contains("--no-wait"), "{}", followed.err);
    let started = std::time::Instant::now();
    while !server.page("garden", "/").body().contains("Published by CI.") {
        assert!(started.elapsed() < Server::PATIENCE, "the queued build never went live: {}", server.said());
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    laptop.write("index.md", "# Garden\n\nPushed without following.\n");
    let queued = ci.ink(&["push", "--no-wait"]);
    assert_eq!(queued.code, 0, "{}", queued.said());
    assert_eq!(queued.out.trim().len(), 32, "the build id alone: {}", queued.out);
    // a deploy key may deploy and may not make a key
    let refused = ci.ink(&["key", "add", "--label", "more", "--poll", "1", "--wait", "5"]);
    assert_ne!(refused.code, 0, "{}", refused.said());

    // revoking the key ends it at once
    let listed = laptop.ink(&["key", "list", "--json"]).json();
    let deploy = listed["keys"].as_array().unwrap().iter().find(|row| row["label"] == "ci").unwrap().clone();
    let revoked = laptop.ink(&["key", "revoke", deploy["fingerprint"].as_str().unwrap()]);
    assert_eq!(revoked.code, 0, "{}", revoked.said());
    laptop.write("index.md", "# Garden\n\nAfter revocation.\n");
    let after = ci.ink(&["push", "--wait", "30"]);
    assert_eq!(after.code, 3, "a revoked key is a sign-in error: {}", after.said());

    // a second person signs in by the email link that the capture mailer prints
    let mut ann = server.browser();
    let asked = ann.send(("POST", "/v1/login"), &json!({ "address": "ann@example.org" }).to_string());
    assert_eq!(asked.status(), "202", "{}", asked.text);
    let stranger =
        server.browser().send(("POST", "/v1/login"), &json!({ "address": "nobody@example.org" }).to_string());
    assert_eq!(stranger.status(), asked.status(), "every address gets one answer");
    let started = std::time::Instant::now();
    while server.links().len() < 3 {
        assert!(started.elapsed() < Server::PATIENCE, "no mail came: {}", server.said());
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let mail = server.links();
    let entered = ann.send(("POST", target(&mail[1])), "");
    assert_eq!(entered.status(), "200", "{}{mail:?}{}", entered.text, server.said());
    ann.keep(&entered);
    assert_eq!(ann.send(("GET", "/v1/session"), "").status(), "200");
    let sessions = ann.send(("GET", "/v1/sessions"), "").json();
    assert_eq!(sessions.as_array().unwrap().len(), 1, "{sessions}");
    // and ends it: the cookie no longer opens anything
    let ended = ann.send(("DELETE", "/v1/session"), "");
    assert_eq!(ended.status(), "200", "{}", ended.text);
    assert_eq!(ann.send(("GET", "/v1/session"), "").status(), "401");

    // logout: the key revokes itself and the machine forgets it
    let out = laptop.ink(&["logout", "--json"]);
    assert_eq!(out.code, 0, "{}", out.said());
    let gone = laptop.ink(&["whoami"]);
    assert_eq!(gone.code, 3, "{}", gone.said());

    // the first-run link is not made again, and the owner's session survives a restart
    let before = server.links().len();
    server.restart();
    assert_eq!(server.links().len(), before, "a restart printed a link: {}", server.said());
    assert!(server.said().contains("not made"), "the start says why there is no link");
    assert_eq!(owner.send(("GET", "/v1/session"), "").status(), "200", "sessions are kept in the database");
    assert!(
        server.page("garden", "/").body().contains("Pushed without following."),
        "the site is served again after a restart"
    );
}
