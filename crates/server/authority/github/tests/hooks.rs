//! A GitHub delivery against recorded bodies and signatures: what is verified, what is refused and
//! what is ignored with a reason.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_github as authority_github;

use authority_forge::{Headers, Host, Ignored, Limits, Received, Refused, Repo, Roster, Secret};
use authority_github::Hooks;
use jiff::{SignedDuration, Timestamp};

const SECRET: &str = "It's a Secret to Everybody";
const PUSH: &[u8] = include_bytes!("fixtures/push.json");
const PUSH_SIG: &str = include_str!("fixtures/push.json.sig");
const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

fn hooks() -> Hooks {
    Hooks::new(Secret::new(SECRET), Limits::DEFAULT)
}

fn roster() -> Roster {
    [Repo::new(Host::GITHUB, "octo/docs".parse().unwrap())].into_iter().collect()
}

fn headers(event: &str, id: &str, signature: &str) -> Headers {
    Headers::new().with("x-github-event", event).with("X-GitHub-Delivery", id).with("X-Hub-Signature-256", signature)
}

#[test]
fn a_signed_push_is_a_typed_push() {
    let got = hooks().receive((&headers("push", "d-1", PUSH_SIG), PUSH), &roster(), now()).unwrap();
    let Received::Push(push) = got else { panic!("{got:?}") };
    assert_eq!(push.repo.to_string(), "github.com/octo/docs");
    assert_eq!((push.branch.as_str(), push.revision.as_str()), ("main", SHA));
}

#[test]
fn a_tampered_body_is_refused() {
    let mut body = PUSH.to_vec();
    let at = body.iter().position(|byte| *byte == b'm').unwrap();
    body[at] = b'M';
    let got = hooks().receive((&headers("push", "d-1", PUSH_SIG), &body), &roster(), now());
    assert!(matches!(got, Err(Refused::Signature)), "{got:?}");
}

#[test]
fn a_wrong_secret_is_refused() {
    let hooks = Hooks::new(Secret::new("another secret"), Limits::DEFAULT);
    let got = hooks.receive((&headers("push", "d-1", PUSH_SIG), PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Signature)), "{got:?}");
}

#[test]
fn a_signature_of_the_wrong_shape_is_refused() {
    let bare = PUSH_SIG.trim().strip_prefix("sha256=").unwrap();
    for bad in [bare, "sha256=", "sha256=zz", "sha1=00", &PUSH_SIG[..PUSH_SIG.len() - 2]] {
        let got = hooks().receive((&headers("push", "d-1", bad), PUSH), &roster(), now());
        assert!(matches!(got, Err(Refused::Signature)), "{bad}: {got:?}");
    }
    let none = Headers::new().with("X-GitHub-Event", "push");
    let got = hooks().receive((&none, PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Missing { header: "X-Hub-Signature-256" })), "{got:?}");
}

#[test]
fn a_replayed_delivery_is_refused_until_the_window_ends() {
    let hooks = hooks();
    let first = headers("push", "d-7", PUSH_SIG);
    hooks.receive((&first, PUSH), &roster(), now()).unwrap();
    let again = hooks.receive((&first, PUSH), &roster(), now() + SignedDuration::from_secs(5));
    assert!(matches!(&again, Err(Refused::Replayed { delivery }) if delivery.to_string() == "d-7"), "{again:?}");
    let later = now() + Limits::DEFAULT.window + SignedDuration::from_secs(1);
    hooks.receive((&first, PUSH), &roster(), later).unwrap();
}

#[test]
fn a_forged_delivery_does_not_use_up_an_id() {
    let hooks = hooks();
    let forged = hooks.receive((&headers("push", "d-8", "sha256=00"), PUSH), &roster(), now());
    assert!(matches!(forged, Err(Refused::Signature)));
    hooks.receive((&headers("push", "d-8", PUSH_SIG), PUSH), &roster(), now()).unwrap();
}

#[test]
fn an_event_for_an_unrelated_repository_is_ignored_with_its_reason() {
    let body = include_bytes!("fixtures/push-other.json");
    let signature = include_str!("fixtures/push-other.json.sig");
    let got = hooks().receive((&headers("push", "d-2", signature), body), &roster(), now()).unwrap();
    let Received::Ignored(Ignored::Unfollowed { repo }) = got else { panic!("{got:?}") };
    assert_eq!(repo.to_string(), "github.com/stranger/elsewhere");
}

#[test]
fn tags_deletions_and_pings_queue_nothing() {
    let tag = (&include_bytes!("fixtures/push-tag.json")[..], include_str!("fixtures/push-tag.json.sig"));
    let gone = (&include_bytes!("fixtures/push-delete.json")[..], include_str!("fixtures/push-delete.json.sig"));
    let ping = (&include_bytes!("fixtures/ping.json")[..], include_str!("fixtures/ping.json.sig"));
    let hooks = hooks();
    let got = hooks.receive((&headers("push", "t-1", tag.1), tag.0), &roster(), now()).unwrap();
    assert!(matches!(got, Received::Ignored(Ignored::Ref { .. })), "{got:?}");
    let got = hooks.receive((&headers("push", "t-2", gone.1), gone.0), &roster(), now()).unwrap();
    assert!(matches!(got, Received::Ignored(Ignored::Deleted { .. })), "{got:?}");
    let got = hooks.receive((&headers("ping", "t-3", ping.1), ping.0), &roster(), now()).unwrap();
    assert_eq!(got, Received::Ignored(Ignored::Event { written: "ping".to_owned() }));
}

#[test]
fn a_body_over_the_limit_is_refused_before_it_is_read() {
    let limits = Limits { body: 16, ..Limits::DEFAULT };
    let hooks = Hooks::new(Secret::new(SECRET), limits);
    let got = hooks.receive((&headers("push", "d-3", "sha256=00"), PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Oversize { limit: 16, .. })), "{got:?}");
}

#[test]
fn a_refusal_carries_a_catalog_code_and_never_the_secret() {
    let err = hooks().receive((&headers("push", "d-4", "sha256=00"), PUSH), &roster(), now()).unwrap_err();
    assert_eq!(err.code(), "hook.signature.invalid");
    assert!(!format!("{err:?} {err}").contains(SECRET));
    assert!(!format!("{:?}", hooks()).contains(SECRET));
}
