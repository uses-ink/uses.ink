//! The operator's side of abuse through the real binary: a stranger reports a site without a
//! key, the operator lists the report, suspends the site (which then serves a refusal) and lifts
//! the suspension; a machine with no operator key is refused.

#![allow(clippy::unwrap_used, clippy::panic, clippy::too_many_lines, reason = "tests stop at the first surprise")]

mod rig;

use rig::Server;
use serde_json::json;

#[test]
fn the_operator_reads_a_report_suspends_the_site_and_lifts_it() {
    let server = Server::start("abuse-operator", &["--site-ttl", "0"]);
    let data = server.data.to_str().unwrap().to_owned();
    let mut operator = server.machine("operator");
    operator.dir = server.root.join("site");
    std::fs::create_dir_all(&operator.dir).unwrap();
    let logged = operator.ink(&["login", "--data", &data, "--label", "operator"]);
    assert_eq!(logged.code, 0, "{}", logged.said());

    operator.write("index.md", "# Garden\n\nHello.\n");
    let init = operator.ink(&["init", "--handle", "garden", "--json"]);
    assert_eq!(init.code, 0, "{}", init.said());
    let site = init.json()["site"].as_str().unwrap().to_owned();
    let pushed = operator.ink(&["push", "--wait", "30"]);
    assert_eq!(pushed.code, 0, "{}", pushed.said());
    assert_eq!(server.page("garden", "/").status(), "200");

    // a stranger reports it, with no key
    let body = json!({ "handle": "garden", "kind": "phishing", "detail": "asks for a bank login" }).to_string();
    let reported = server.http(("POST", "/v1/abuse"), &[("Content-Type", "application/json")], &body);
    assert_eq!(reported.status(), "201", "{}", reported.text);
    let report = reported.json()["id"].as_str().unwrap().to_owned();

    let listed = operator.ink(&["abuse", "list", "--json"]);
    assert_eq!(listed.code, 0, "{}", listed.said());
    let rows = listed.json()["reports"].as_array().unwrap().clone();
    assert_eq!((rows.len(), rows[0]["handle"].as_str(), rows[0]["state"].as_str()), (1, Some("garden"), Some("open")));
    let plain = operator.ink(&["abuse", "list"]);
    assert!(plain.out.contains("garden") && plain.out.contains("phishing"), "{}", plain.said());

    let suspended = operator.ink(&["abuse", "suspend", &site, "--kind", "phishing", "--report", &report, "--json"]);
    assert_eq!(suspended.code, 0, "{}", suspended.said());
    assert_eq!(suspended.json()["kind"], "phishing");
    let page = server.page("garden", "/");
    assert_eq!(page.status(), "403", "a suspended site serves a refusal: {}", page.text);
    let after = operator.ink(&["abuse", "list", "--json"]);
    assert_eq!(after.json()["reports"][0]["state"], "actioned", "{}", after.out);

    let restored = operator.ink(&["abuse", "reinstate", &site, "--json"]);
    assert_eq!(restored.code, 0, "{}", restored.said());
    assert_eq!(restored.json()["kind"], serde_json::Value::Null);
    assert_eq!(server.page("garden", "/").status(), "200");
}

#[test]
fn a_machine_with_no_key_or_a_bad_kind_is_refused_before_anything_changes() {
    let server = Server::start("abuse-refused", &["--site-ttl", "0"]);
    let nobody = server.machine("nobody");
    let site = "01".repeat(16);
    let listed = nobody.ink(&["abuse", "list"]);
    assert_eq!(listed.code, 3, "no key is a sign-in error: {}", listed.said());
    let unknown = nobody.ink(&["abuse", "suspend", &site, "--kind", "rude"]);
    assert_eq!(unknown.code, 2, "{}", unknown.said());
    assert!(unknown.err.contains("is no kind of abuse"), "{}", unknown.err);
    let wrong = nobody.ink(&["abuse", "reinstate", "not-an-id"]);
    assert_eq!(wrong.code, 2, "{}", wrong.said());
}
