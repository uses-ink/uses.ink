//! Passkeys through the real binary, with a software authenticator that builds what a browser
//! would send: register, sign in with no username, step-up, the list of sessions and ending one,
//! recovery codes minted and redeemed, the Origin refusals and the limits. The rules themselves
//! are held by `edge-api`'s tests; this holds that `ink serve --data` wires them as documented.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

#[path = "../../../server/edge/api/tests/support/authenticator.rs"]
mod authenticator;
mod rig;

use authenticator::Authenticator;
use rig::{Browser, Server};
use serde_json::{Value, json};

const HOST: &str = "api.localhost";

/// A browser signed in by the first-run link: the account's first session.
fn owner(server: &Server) -> Browser<'_> {
    let links = server.links();
    let link = &links[0];
    let mut browser = server.browser();
    let entered = browser.send(("POST", &format!("/v1{}", &link[link.find("/login/").unwrap()..])), "");
    assert_eq!(entered.status(), "200", "{}", entered.text);
    browser.keep(&entered);
    browser
}

/// Registers `authenticator` as `label` for the account of `browser`.
fn register(browser: &mut Browser<'_>, authenticator: &Authenticator, label: &str) -> rig::Got {
    let options = browser.send(("POST", "/v1/passkeys/options"), &json!({ "label": label }).to_string());
    if options.status() != "200" {
        return options;
    }
    browser.keep(&options);
    let credential = authenticator.create(&options.json());
    let added =
        browser.send(("POST", "/v1/passkeys"), &json!({ "label": label, "credential": credential }).to_string());
    browser.keep(&added);
    added
}

/// A whole passkey sign-in in a fresh browser: the browser, and the reply of the last step.
fn sign_in<'a>(server: &'a Server, authenticator: &Authenticator, handle: &[u8]) -> (Browser<'a>, rig::Got) {
    let mut browser = server.browser();
    let challenge = browser.send(("POST", "/v1/passkeys/challenge"), "");
    assert_eq!(challenge.status(), "200", "{}", challenge.text);
    browser.keep(&challenge);
    let credential = authenticator.get(&challenge.json(), handle);
    let signed = browser.send(("POST", "/v1/passkeys/session"), &json!({ "credential": credential }).to_string());
    browser.keep(&signed);
    (browser, signed)
}

#[test]
fn passkeys_register_sign_in_step_up_and_recover_on_the_real_server() {
    let server = Server::start("passkey", &[]);
    let origin = server.api.clone();
    let mut link = owner(&server);
    assert!(server.said().contains("relying party id api.localhost"), "the start says who passkeys are for");

    // the options are the ones documented
    let options = link.send(("POST", "/v1/passkeys/options"), &json!({ "label": "phone" }).to_string());
    assert_eq!(options.status(), "200", "{}", options.text);
    let shown = options.json();
    assert_eq!(shown["rp"]["id"], HOST);
    assert_eq!(shown["attestation"], "none");
    assert_eq!(shown["authenticatorSelection"], json!({"residentKey": "required", "userVerification": "required"}));
    let ceremony = options.set_cookie();
    assert!(ceremony.starts_with("ink-ceremony=") && ceremony.contains("HttpOnly"), "{ceremony}");
    let handle = sig::Base64::decode_vec(shown["user"]["id"].as_str().unwrap()).unwrap();

    // a cross-origin page cannot start or finish anything, and a wrong claim is refused whole
    let cross = link.send_with(
        ("POST", "/v1/passkeys/options"),
        &[("Origin", "https://evil.example")],
        &json!({ "label": "x" }).to_string(),
    );
    assert_eq!((cross.status(), cross.problem().as_str()), ("403", "auth.origin.refused"), "{}", cross.text);
    let elsewhere = Authenticator::synced(1, "https://evil.example", HOST);
    let refused = register(&mut link, &elsewhere, "phone");
    assert!(refused.status().starts_with('4'), "an origin claimed in the client data: {}", refused.text);
    let other = Authenticator::synced(1, &origin, "evil.example");
    let refused = register(&mut link, &other, "phone");
    assert!(refused.status().starts_with('4'), "another relying party id: {}", refused.text);

    // the phone registers; the list shows it and no key material
    let phone = Authenticator::synced(1, &origin, HOST);
    let added = register(&mut link, &phone, "phone");
    assert_eq!(added.status(), "201", "{}", added.text);
    let listed = link.send(("GET", "/v1/passkeys"), "").json();
    assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(listed[0]["label"], "phone");
    assert!(listed[0].get("key").is_none(), "{listed}");

    // with a passkey on the account, the email link's session is no step-up
    let laptop = Authenticator::hardware(2, &origin, HOST);
    let stale = register(&mut link, &laptop, "laptop");
    assert_eq!((stale.status(), stale.problem().as_str()), ("403", "auth.step-up.required"), "{}", stale.text);
    let (mut fresh, signed) = sign_in(&server, &phone, &handle);
    assert_eq!(signed.status(), "200", "{}", signed.text);
    assert!(signed.json().get("session").is_none(), "the session is a cookie only: {}", signed.text);
    let added = register(&mut fresh, &laptop, "laptop");
    assert_eq!(added.status(), "201", "a passkey session is a step-up: {}", added.text);

    // the sessions: two of the account, the asking one marked, any one ended by id
    let sessions = fresh.send(("GET", "/v1/sessions"), "").json();
    let rows = sessions.as_array().unwrap();
    assert!(rows.len() >= 2, "{sessions}");
    let current = rows.iter().filter(|row| row["current"] == json!(true)).count();
    assert_eq!(current, 1, "{sessions}");
    let theirs = rows.iter().find(|row| row["current"] == json!(false)).unwrap()["id"].as_str().unwrap().to_owned();
    let ended = fresh.send(("delete", &format!("/v1/sessions/{theirs}")), "");
    assert_eq!(ended.status(), "200", "{}", ended.text);
    assert_eq!(link.send(("GET", "/v1/session"), "").status(), "401", "the ended session no longer opens anything");
    assert_eq!(fresh.send(("GET", "/v1/session"), "").status(), "200");
    let missing = fresh.send(("delete", &format!("/v1/sessions/{theirs}")), "");
    assert_eq!((missing.status(), missing.problem().as_str()), ("404", "auth.session.missing"));

    // recovery codes: ten, shown once, and a new set voids the old
    let first = fresh.send(("POST", "/v1/recovery/codes"), "");
    assert_eq!(first.status(), "201", "{}", first.text);
    let codes: Vec<String> =
        first.json()["codes"].as_array().unwrap().iter().map(|code| code.as_str().unwrap().to_owned()).collect();
    assert_eq!(codes.len(), 10);
    assert_eq!(fresh.send(("GET", "/v1/recovery/codes"), "").json()["remaining"], 10);
    assert!(!server.said().contains(&codes[0]), "a recovery code reached the server's output");
    let second = fresh.send(("POST", "/v1/recovery/codes"), "");
    assert_eq!(second.status(), "201", "{}", second.text);
    let mut nobody = server.browser();
    let voided = nobody.send(("POST", "/v1/recovery"), &json!({ "code": codes[0], "label": "new" }).to_string());
    assert_eq!((voided.status(), voided.problem().as_str()), ("401", "auth.recovery.refused"), "{}", voided.text);
    let current: Vec<String> =
        second.json()["codes"].as_array().unwrap().iter().map(|code| code.as_str().unwrap().to_owned()).collect();

    // a lost phone: a code with no session registers a new passkey, once
    let started =
        nobody.send(("POST", "/v1/recovery"), &json!({ "code": current[0], "label": "new phone" }).to_string());
    assert_eq!(started.status(), "200", "{}", started.text);
    nobody.keep(&started);
    let spare = Authenticator::hardware(5, &origin, HOST);
    let credential = spare.create(&started.json());
    let finished =
        nobody.send(("POST", "/v1/passkeys"), &json!({ "label": "new phone", "credential": credential }).to_string());
    assert_eq!(finished.status(), "201", "{}", finished.text);
    assert_eq!(sign_in(&server, &spare, &handle).1.status(), "200", "the recovered passkey signs in");
    let again =
        server.browser().send(("POST", "/v1/recovery"), &json!({ "code": current[0], "label": "x" }).to_string());
    assert_eq!((again.status(), again.problem().as_str()), ("401", "auth.recovery.refused"), "a code is spent once");
    assert_eq!(fresh.send(("GET", "/v1/recovery/codes"), "").json()["remaining"], 9);

    // a code that is well formed and ours to refuse counts against the source: ten an hour
    let never = control::recovery::Code::from_bytes([9; 16]).reveal();
    let mut limited = Value::Null;
    for _ in 0..12 {
        let miss =
            server.browser().send(("POST", "/v1/recovery"), &json!({ "code": &never, "label": "x" }).to_string());
        if miss.status() == "429" {
            limited = json!(miss.problem());
            assert!(miss.header("retry-after").is_some(), "a limit says when to come back: {}", miss.text);
            break;
        }
    }
    assert_eq!(limited, json!("auth.rate.limited"));
}

#[test]
fn starting_passkey_sign_ins_is_limited_per_source() {
    let server = Server::start("challenge", &["--challenge-limit", "3/1h"]);
    let mut statuses = Vec::new();
    for _ in 0..5 {
        statuses.push(server.browser().send(("POST", "/v1/passkeys/challenge"), "").status().to_owned());
    }
    assert_eq!(statuses, ["200", "200", "200", "429", "429"]);
    let limited = server.browser().send(("POST", "/v1/passkeys/challenge"), "");
    assert_eq!(limited.problem(), "auth.rate.limited", "{}", limited.text);
    assert!(limited.header("retry-after").is_some(), "{}", limited.text);
}
