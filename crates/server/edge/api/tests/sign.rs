//! Signed requests on the API: every credential kind, replay, expiry, tamper, enumeration, skew,
//! a restart, revocation, enrolment and its limits, key listing, and a golden for each shape of
//! refusal. Time is a manual clock, so each of these is exact.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_edge_api as edge_api;

mod support;

use std::time::Duration;

use edge_api::{
    Credentials, Limits, Method, Offered, OperatorFile, Presented, Reply, Signatures, Signing, Status, Verdict, Window,
};
use serde_json::{Value, json};
use support::{AUDIENCE, Rig, SKEW, signer};

fn rig() -> Rig {
    Rig::new(Limits::default())
}

fn code(reply: &Reply) -> String {
    serde_json::from_slice::<Value>(&reply.body).unwrap()["problem"].as_str().unwrap_or("").to_owned()
}

fn text(reply: &Reply) -> String {
    String::from_utf8(reply.body.clone()).unwrap()
}

/// A site of the account, added as the operator.
fn site(rig: &Rig, handle: &str) -> String {
    let body = json!({ "handle": handle, "source": { "kind": "upload" } }).to_string();
    let (reply, value) = rig.call(Method::Post, "/v1/sites", body.as_bytes());
    assert_eq!(reply.status, Status::Created, "{value}");
    value["id"].as_str().unwrap().to_owned()
}

#[test]
fn each_kind_of_credential_is_who_it_says_and_has_what_it_was_given() {
    let rig = rig();
    let garden = site(&rig, "garden");
    let wild = site(&rig, "wild");
    let (device, deploy) = (signer(1), signer(2));
    rig.enrolled(&device, &json!({}));
    rig.enrolled(&deploy, &json!({ "purpose": "deploy", "site": garden }));

    let (reply, who) = rig.call(Method::Get, "/v1/whoami", b"");
    assert_eq!((reply.status, who["kind"].as_str()), (Status::Ok, Some("operator")));
    let (_, who) = rig.call_as(&device, Method::Get, "/v1/whoami", b"");
    assert_eq!(who["kind"], "device");
    assert_eq!(who["fingerprint"], device.id().fingerprint().to_string());
    assert_eq!(who["grants"][0]["rights"], json!(["read", "deploy", "edit", "keys"]));
    let (_, who) = rig.call_as(&deploy, Method::Get, "/v1/whoami", b"");
    assert_eq!((who["kind"].clone(), who["grants"][0]["scope"].clone()), (json!("deploy"), json!("site")));

    // a device key reads, deploys and edits, and may not destroy
    let (reply, _) = rig.call_as(&device, Method::Get, "/v1/sites", b"");
    assert_eq!(reply.status, Status::Ok);
    let (reply, _) = rig.call_as(&device, Method::Post, &format!("/v1/sites/{garden}/resume"), b"");
    assert_eq!(reply.status, Status::Ok);
    let (reply, _) = rig.call_as(&device, Method::Delete, &format!("/v1/sites/{garden}"), b"");
    assert_eq!((reply.status, code(&reply)), (Status::Forbidden, "auth.right.missing".to_owned()));
    // a deploy key deploys on its one site and does nothing else
    let (reply, _) = rig.call_as(&deploy, Method::Post, &format!("/v1/sites/{garden}/resume"), b"");
    assert_eq!(reply.status, Status::Ok);
    let (reply, _) = rig.call_as(&deploy, Method::Post, &format!("/v1/sites/{wild}/resume"), b"");
    assert_eq!(reply.status, Status::Forbidden);
    let (reply, _) = rig.call_as(&deploy, Method::Get, "/v1/sites", b"");
    assert_eq!(reply.status, Status::Forbidden);
    let (reply, _) = rig.call_as(&deploy, Method::Get, &format!("/v1/sites/{garden}"), b"");
    assert_eq!(reply.status, Status::Forbidden, "a deploy key has no read");
    // the operator may do all of it
    let (reply, _) = rig.call(Method::Delete, &format!("/v1/sites/{garden}"), b"");
    assert_eq!(reply.status, Status::Ok);
}

#[test]
fn a_request_is_taken_once_and_the_replay_is_recorded_for_a_key() {
    let rig = rig();
    let device = signer(1);
    rig.enrolled(&device, &json!({}));
    let request = rig.request(&device, (Method::Get, "/v1/whoami"), b"");
    assert_eq!(rig.answer(&request).status, Status::Ok);
    let replay = rig.answer(&request);
    assert_eq!((replay.status, code(&replay)), (Status::Unauthorized, "auth.signature.replayed".to_owned()));
    let events = pollster::block_on(rig.enrol.keys().events(rig.account, 10)).unwrap();
    assert!(events.iter().any(|event| event.name == "replay.detected"));
    // the operator's replay is refused the same way
    let request = rig.request(&rig.operator, (Method::Get, "/v1/whoami"), b"");
    assert_eq!(rig.answer(&request).status, Status::Ok);
    assert_eq!(code(&rig.answer(&request)), "auth.signature.replayed");
}

#[test]
fn a_key_past_its_end_says_so_once_it_has_proved_itself() {
    let rig = rig();
    let device = signer(1);
    rig.enrolled(&device, &json!({ "lifetime": "1m" }));
    assert_eq!(rig.call_as(&device, Method::Get, "/v1/whoami", b"").0.status, Status::Ok);
    rig.clock.advance(Duration::from_secs(61));
    let (reply, problem) = rig.call_as(&device, Method::Get, "/v1/whoami", b"");
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.key.expired".to_owned()));
    assert!(problem["fields"]["since"].is_string());
    // a stranger's key learns nothing of it
    let (reply, _) = rig.call_as(&signer(7), Method::Get, "/v1/whoami", b"");
    assert_eq!(code(&reply), "auth.signature.refused");
}

#[test]
fn revoking_a_key_ends_it_and_a_key_may_revoke_itself() {
    let rig = rig();
    let (laptop, ci) = (signer(1), signer(2));
    rig.enrolled(&laptop, &json!({}));
    rig.enrolled(&ci, &json!({}));
    let print = ci.id().fingerprint().to_string();
    let (reply, _) = rig.call_as(&laptop, Method::Delete, &format!("/v1/keys/{print}"), b"");
    assert_eq!(reply.status, Status::Ok);
    let (reply, problem) = rig.call_as(&ci, Method::Get, "/v1/whoami", b"");
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.key.revoked".to_owned()));
    assert!(problem["fields"]["since"].is_string());
    // a second revoke is no key
    let (reply, _) = rig.call_as(&laptop, Method::Delete, &format!("/v1/keys/{print}"), b"");
    assert_eq!((reply.status, code(&reply)), (Status::NotFound, "auth.key.unknown".to_owned()));
    // logout: a key revokes itself, and the event says it was a logout
    let own = laptop.id().fingerprint().to_string();
    assert_eq!(rig.call_as(&laptop, Method::Delete, &format!("/v1/keys/{own}"), b"").0.status, Status::Ok);
    assert_eq!(code(&rig.call_as(&laptop, Method::Get, "/v1/whoami", b"").0), "auth.key.revoked");
}

#[test]
fn keys_are_listed_with_a_day_and_no_secret() {
    let rig = rig();
    let device = signer(1);
    rig.enrolled(&device, &json!({}));
    rig.call_as(&device, Method::Get, "/v1/whoami", b"");
    let (reply, listed) = rig.call_as(&device, Method::Get, "/v1/keys", b"");
    assert_eq!(reply.status, Status::Ok);
    let key = &listed["items"][0];
    assert_eq!(key["fingerprint"], device.id().fingerprint().to_string());
    assert_eq!(key["last_used"], "2027-01-15");
    assert!(key["revoked"].is_null());
    let shown = text(&reply);
    assert!(!shown.contains(&device.public().to_string()), "a listing holds no public key either: {shown}");
    // a deploy key has no `keys` right
    let deploy = signer(2);
    let garden = site(&rig, "garden");
    rig.enrolled(&deploy, &json!({ "purpose": "deploy", "site": garden }));
    assert_eq!(rig.call_as(&deploy, Method::Get, "/v1/keys", b"").0.status, Status::Forbidden);
}

#[test]
fn skew_is_a_window_either_side_and_says_what_the_server_thinks() {
    let rig = rig();
    let route = (Method::Get, "/v1/whoami");
    let now = rig.now();
    for (created, taken) in [(now - SKEW, true), (now + SKEW, true), (now - SKEW - 1, false), (now + SKEW + 1, false)] {
        let header = Rig::header(&rig.operator, route, b"", (created, rig.nonce()));
        let reply = rig.answer(&Rig::unsigned(route, Some(header), b""));
        assert_eq!(reply.status == Status::Ok, taken, "{created} against {now}: {}", text(&reply));
        if !taken {
            let problem: Value = serde_json::from_slice(&reply.body).unwrap();
            assert_eq!(problem["problem"], "auth.signature.skewed");
            assert_eq!(problem["fields"]["window"], SKEW.to_string());
            assert!(problem["fields"]["server_time"].as_str().unwrap().starts_with("2027-01-15T"));
        }
    }
}

#[test]
fn a_restart_does_not_take_what_was_signed_before_it() {
    let rig = rig();
    let route = (Method::Get, "/v1/whoami");
    let captured = rig.request(&rig.operator, route, b"");
    assert_eq!(rig.answer(&captured).status, Status::Ok);
    // the same server, started again: an empty nonce store
    rig.clock.advance(Duration::from_secs(1));
    let file = OperatorFile::at(rig.folder.path().join("operator-keys"));
    let mut config = Signing::new(sig::Audience::parse(AUDIENCE).unwrap(), rig.account);
    config.window = Window::new(SKEW).unwrap();
    let again = Signatures::new(rig.enrol.keys().clone(), (rig.clock, file), config);
    let signed = sig::Signed::parse(captured.authorization.as_deref().unwrap()).unwrap();
    let (method, target) = (sig::Method::parse("GET").unwrap(), sig::Target::parse("/v1/whoami").unwrap());
    let presented = Presented { method: &method, target: &target };
    let verdict = pollster::block_on(again.verify(&Offered::Signed(signed), &presented)).unwrap();
    let Verdict::Refused(denied) = verdict else { panic!("a replay after a restart was taken") };
    assert_eq!(denied.code(), "auth.server.restarting");
    // once the quiet is over, what is signed then is taken
    rig.clock.advance(Duration::from_secs(SKEW + 1));
    let fresh = rig.request(&rig.operator, route, b"");
    let signed = sig::Signed::parse(fresh.authorization.as_deref().unwrap()).unwrap();
    assert!(matches!(pollster::block_on(again.verify(&Offered::Signed(signed), &presented)), Ok(Verdict::Granted(_))));
    // and the old one is outside the window, not replayed into the new store
    let signed = sig::Signed::parse(captured.authorization.as_deref().unwrap()).unwrap();
    let Ok(Verdict::Refused(denied)) = pollster::block_on(again.verify(&Offered::Signed(signed), &presented)) else {
        panic!("the old request was taken")
    };
    assert_eq!(denied.code(), "auth.signature.skewed");
}

/// The same header with one field's value changed to another of its kind.
fn changed(header: &[u8], field: &str) -> Vec<u8> {
    let text = String::from_utf8(header.to_vec()).unwrap();
    let (scheme, rest) = text.split_once(' ').unwrap();
    let parts: Vec<String> = rest
        .split(", ")
        .map(|part| {
            let (name, value) = part.split_once('=').unwrap();
            if name != field {
                return part.to_owned();
            }
            let value = if name == "c" {
                (value.parse::<u64>().unwrap() + 1).to_string()
            } else {
                let mut bytes = value.as_bytes().to_vec();
                bytes[3] = if bytes[3] == b'A' { b'B' } else { b'A' };
                String::from_utf8(bytes).unwrap()
            };
            format!("{name}={value}")
        })
        .collect();
    format!("{scheme} {}", parts.join(", ")).into_bytes()
}

#[test]
fn every_component_is_signed_and_what_fails_is_one_answer() {
    let rig = rig();
    let route = (Method::Get, "/v1/whoami");
    let good = rig.request(&rig.operator, route, b"");
    let header = good.authorization.clone().unwrap();
    let refused = |request: &edge_api::Request| rig.answer(request);
    // a changed key, time, nonce, digest or signature
    let mut replies = Vec::new();
    for field in ["k", "c", "n", "d", "s"] {
        replies.push((field, refused(&Rig::unsigned(route, Some(changed(&header, field)), b""))));
    }
    // another method or target under the same header
    let other = Method::Post;
    replies.push(("method", refused(&Rig::unsigned((other, "/v1/whoami"), Some(header.clone()), b""))));
    replies.push(("target", refused(&Rig::unsigned((Method::Get, "/v1/keys"), Some(header.clone()), b""))));
    // another audience, and another key's signature
    let wrong_audience = {
        let audience = sig::Audience::parse("other.example").unwrap();
        let (method, target) = (sig::Method::parse("GET").unwrap(), sig::Target::parse("/v1/whoami").unwrap());
        let parts = sig::Parts { audience: &audience, method: &method, target: &target, body: b"" };
        rig.operator.sign(&parts, sig::Created::new(rig.now()), rig.nonce()).to_string().into_bytes()
    };
    replies.push(("audience", refused(&Rig::unsigned(route, Some(wrong_audience), b""))));
    let stranger = rig.request(&signer(55), route, b"");
    replies.push(("unknown key", refused(&stranger)));
    for (what, reply) in &replies {
        assert_eq!(code(reply), "auth.signature.refused", "{what}");
    }
    // enumeration: unknown key, wrong signature and wrong audience are one answer, byte for byte
    let first = &replies[0].1;
    let wrong_signature = &replies[4].1;
    for (what, reply) in &replies[1..] {
        assert_eq!(reply, wrong_signature, "{what}");
    }
    assert_eq!(first, wrong_signature);
    // a body that is not the signed one is its own answer, after the signature verified
    let signed_for = Rig::header(&rig.operator, (Method::Post, "/v1/sites"), b"{}", (rig.now(), rig.nonce()));
    let reply = refused(&Rig::unsigned((Method::Post, "/v1/sites"), Some(signed_for), b"{ }"));
    assert_eq!((reply.status, code(&reply)), (Status::BadRequest, "auth.digest.mismatch".to_owned()));
    // the original, once, still works: none of that spent its nonce
    assert_eq!(rig.answer(&good).status, Status::Ok);
}

#[test]
fn what_is_not_canonical_or_not_a_signature_is_refused_as_such() {
    let rig = rig();
    let header = Rig::header(&rig.operator, (Method::Get, "/v1/sites"), b"", (rig.now(), rig.nonce()));
    let mut request = Rig::unsigned((Method::Get, "/v1/sites"), Some(header.clone()), b"");
    request.query = Some("b=1&a=2".into());
    let reply = rig.answer(&request);
    assert_eq!((reply.status, code(&reply)), (Status::BadRequest, "auth.target.not-canonical".to_owned()));
    for bad in [&b"Ink-Sig-1 k=x"[..], b"Ink-Sig-1", b"Bearer abc", b"", b"Ink-Sig-1 \xff"] {
        let reply = rig.answer(&Rig::unsigned((Method::Get, "/v1/sites"), Some(bad.to_vec()), b""));
        assert_eq!(code(&reply), "auth.signature.malformed", "{bad:?}");
    }
    // the header with its one canonical spelling changed is not parsed
    let loose = String::from_utf8(header).unwrap().replace(", ", ",");
    let reply = rig.answer(&Rig::unsigned((Method::Get, "/v1/sites"), Some(loose.into_bytes()), b""));
    assert_eq!(code(&reply), "auth.signature.malformed");
}

#[test]
fn the_passkey_is_reserved_and_refused_as_a_bad_signature_is() {
    let rig = rig();
    let file = OperatorFile::at(rig.folder.path().join("operator-keys"));
    let config = Signing::new(sig::Audience::parse(AUDIENCE).unwrap(), rig.account);
    let verifier = Signatures::new(rig.enrol.keys().clone(), (rig.clock, file), config);
    let (method, target) = (sig::Method::parse("GET").unwrap(), sig::Target::parse("/v1/sites").unwrap());
    let verdict =
        pollster::block_on(verifier.verify(&Offered::Passkey, &Presented { method: &method, target: &target }));
    assert!(matches!(verdict, Ok(Verdict::Refused(edge_api::Denied::Refused))));
}

#[test]
fn a_key_enrols_by_a_code_a_person_approves_and_polls_with_its_own_signature() {
    let rig = rig();
    let newcomer = signer(9);
    let (id, user_code) = rig.ask(&newcomer, &json!({ "label": "laptop" }));
    assert_eq!(user_code.len(), 9, "8 letters and a dash: {user_code}");
    let poll = format!("/v1/enrol/{id}");
    let (reply, polled) = rig.call_as(&newcomer, Method::Get, &poll, b"");
    assert_eq!((reply.status, polled["state"].clone()), (Status::Ok, json!("pending")));
    // another key may not read it, and nothing else may be done by a key not yet enrolled
    assert_eq!(code(&rig.call_as(&signer(8), Method::Get, &poll, b"").0), "auth.signature.refused");
    let (reply, _) = rig.call_as(&newcomer, Method::Get, "/v1/sites", b"");
    assert_eq!((reply.status, code(&reply)), (Status::Unauthorized, "auth.signature.refused".to_owned()));
    // approval makes it a key; its poll then says approved
    let approval = control::keys::Approval {
        code: user_code.parse().unwrap(),
        fingerprint: newcomer.id().fingerprint(),
        by: rig.account,
        within: control::keys::Rights::EVERY,
    };
    let now = jiff::Timestamp::from_second(i64::try_from(rig.now()).unwrap()).unwrap();
    pollster::block_on(rig.enrol.keys().approve(approval, now)).unwrap();
    let (reply, polled) = rig.call_as(&newcomer, Method::Get, &poll, b"");
    assert_eq!((reply.status, polled["state"].clone()), (Status::Ok, json!("approved")));
    assert_eq!(rig.call_as(&newcomer, Method::Get, "/v1/sites", b"").0.status, Status::Ok);
    // a turned-down ask says so
    let other = signer(10);
    let (id, user_code) = rig.ask(&other, &json!({}));
    pollster::block_on(rig.enrol.keys().deny(&user_code.parse().unwrap(), now)).unwrap();
    let (reply, _) = rig.call_as(&other, Method::Get, &format!("/v1/enrol/{id}"), b"");
    assert_eq!((reply.status, code(&reply)), (Status::Forbidden, "auth.enrolment.denied".to_owned()));
}

#[test]
fn enrolling_is_closed_when_too_many_wait_and_the_ask_is_checked() {
    let rig = rig();
    for n in 0..20_u8 {
        rig.ask(&signer(100 + n), &json!({}));
    }
    let body = json!({ "key": signer(200).public().to_string(), "label": "late", "purpose": "device" }).to_string();
    let reply = rig.answer(&Rig::unsigned((Method::Post, "/v1/enrol"), None, body.as_bytes()));
    assert_eq!((reply.status, code(&reply)), (Status::TooMany, "auth.enrolment.limited".to_owned()));
    assert_eq!(reply.retry, Some(Duration::from_secs(60)));
    // closed means closed for a minute, and then open again once the others have expired
    rig.clock.advance(Duration::from_mins(11));
    let reply = rig.answer(&Rig::unsigned((Method::Post, "/v1/enrol"), None, body.as_bytes()));
    assert_eq!(reply.status, Status::Created);
    // a bad ask is one that says what is wrong
    for bad in [
        json!({}),
        json!({ "key": "x", "label": "l", "purpose": "device" }),
        json!({ "key": signer(1).public().to_string(), "label": "l", "purpose": "root" }),
    ] {
        let reply = rig.answer(&Rig::unsigned((Method::Post, "/v1/enrol"), None, bad.to_string().as_bytes()));
        assert_eq!(reply.status, Status::BadRequest, "{bad}");
    }
    let deploy = json!({ "key": signer(1).public().to_string(), "label": "ci", "purpose": "deploy" });
    let reply = rig.answer(&Rig::unsigned((Method::Post, "/v1/enrol"), None, deploy.to_string().as_bytes()));
    assert_eq!((reply.status, code(&reply)), (Status::Unprocessable, "auth.enrolment.site".to_owned()));
}

#[test]
fn one_source_asking_to_enrol_cannot_close_the_gate_for_the_others() {
    let rig = Rig::strict();
    let ask = |n: u8, from: &str| {
        let body = json!({ "key": signer(n).public().to_string(), "label": "k", "purpose": "device" }).to_string();
        let mut request = Rig::unsigned((Method::Post, "/v1/enrol"), None, body.as_bytes());
        request.source = Some(from.parse().unwrap());
        rig.answer(&request)
    };
    // the flood is turned away by its own source's limit, long before 20 keys wait
    let outcomes: Vec<(Status, String)> =
        (0..30).map(|n| ask(100 + n, "203.0.113.9")).map(|reply| (reply.status, code(&reply))).collect();
    let made = outcomes.iter().filter(|(status, _)| *status == Status::Created).count();
    assert_eq!(made, 5);
    assert!(outcomes.iter().skip(5).all(|outcome| *outcome == (Status::TooMany, "auth.enrolment.limited".to_owned())));
    // and everyone else is still taken
    assert_eq!(ask(200, "198.51.100.1").status, Status::Created);
}

#[test]
fn each_shape_of_refusal_is_a_golden() {
    let rig = rig();
    let route = (Method::Get, "/v1/whoami");
    let goldens = [
        (
            rig.answer(&Rig::unsigned(
                route,
                Some(changed(&Rig::header(&rig.operator, route, b"", (rig.now(), rig.nonce())), "s")),
                b"",
            )),
            (
                401,
                r#"{"problem":"auth.signature.refused","message":"the server did not accept the signature","causes":[]}"#,
            ),
        ),
        (
            rig.answer(&Rig::unsigned(route, None, b"")),
            (
                400,
                r#"{"problem":"auth.signature.malformed","message":"sign the request: `Authorization: Ink-Sig-1 ...`","causes":[]}"#,
            ),
        ),
        (
            {
                let mut request = Rig::unsigned(route, None, b"");
                request.authorization = Some(Rig::header(&rig.operator, route, b"", (rig.now(), rig.nonce())));
                request.query = Some("z".into());
                rig.answer(&request)
            },
            (
                400,
                r#"{"problem":"auth.target.not-canonical","message":"the request target is not in canonical form","causes":[]}"#,
            ),
        ),
        (
            rig.answer(&Rig::unsigned(
                route,
                Some(Rig::header(&rig.operator, route, b"", (rig.now() + 100, rig.nonce()))),
                b"",
            )),
            (
                401,
                r#"{"problem":"auth.signature.skewed","message":"the request's time is 100 seconds from the server's, over the allowed 5","causes":[],"fields":{"server_time":"2027-01-15T08:00:11Z","skew":"100","window":"5"}}"#,
            ),
        ),
    ];
    for (reply, (status, body)) in goldens {
        assert_eq!((reply.status.code(), text(&reply).as_str()), (status, body));
        assert!(reply.challenge);
    }
}
