//! A Forgejo delivery against recorded bodies and signatures.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_forgejo as authority_forgejo;

use authority_forge::{Headers, Host, Ignored, Limits, Received, Refused, Repo, Roster, Secret};
use authority_forgejo::Hooks;
use jiff::{SignedDuration, Timestamp};

const SECRET: &str = "forgejo-hook-secret";
const PUSH: &[u8] = include_bytes!("fixtures/push.json");
const PUSH_SIG: &str = include_str!("fixtures/push.json.sig");
const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

fn codeberg() -> Host {
    "codeberg.org".parse().unwrap()
}

fn hooks() -> Hooks {
    Hooks::new(codeberg(), Secret::new(SECRET), Limits::DEFAULT)
}

fn roster() -> Roster {
    [Repo::new(codeberg(), "octo/docs".parse().unwrap())].into_iter().collect()
}

fn forgejo(event: &str, id: &str, signature: &str) -> Headers {
    Headers::new().with("X-Forgejo-Event", event).with("X-Forgejo-Delivery", id).with("X-Forgejo-Signature", signature)
}

#[test]
fn a_signed_push_is_a_typed_push_under_either_spelling() {
    let gitea = Headers::new()
        .with("x-gitea-event", "push")
        .with("x-gitea-delivery", "g-1")
        .with("x-gitea-signature", PUSH_SIG);
    for headers in [forgejo("push", "f-1", PUSH_SIG), gitea] {
        let got = hooks().receive((&headers, PUSH), &roster(), now()).unwrap();
        let Received::Push(push) = got else { panic!("{got:?}") };
        assert_eq!(push.repo.to_string(), "codeberg.org/octo/docs");
        assert_eq!((push.branch.as_str(), push.revision.as_str()), ("main", SHA));
    }
}

#[test]
fn a_tampered_body_or_a_wrong_secret_is_refused() {
    let mut body = PUSH.to_vec();
    let at = body.iter().position(|byte| *byte == b'm').unwrap();
    body[at] = b'M';
    let got = hooks().receive((&forgejo("push", "f-2", PUSH_SIG), &body), &roster(), now());
    assert!(matches!(got, Err(Refused::Signature)), "{got:?}");
    let other = Hooks::new(codeberg(), Secret::new("nope"), Limits::DEFAULT);
    let got = other.receive((&forgejo("push", "f-2", PUSH_SIG), PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Signature)), "{got:?}");
    let got = hooks().receive((&forgejo("push", "f-2", "sha256=00"), PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Signature)), "{got:?}");
}

#[test]
fn a_replayed_delivery_is_refused() {
    let hooks = hooks();
    let headers = forgejo("push", "f-3", PUSH_SIG);
    hooks.receive((&headers, PUSH), &roster(), now()).unwrap();
    let again = hooks.receive((&headers, PUSH), &roster(), now() + SignedDuration::from_secs(1));
    assert!(matches!(again, Err(Refused::Replayed { .. })), "{again:?}");
}

#[test]
fn unrelated_repositories_deletions_and_other_events_are_ignored() {
    let hooks = hooks();
    let other = (&include_bytes!("fixtures/push-other.json")[..], include_str!("fixtures/push-other.json.sig"));
    let gone = (&include_bytes!("fixtures/push-delete.json")[..], include_str!("fixtures/push-delete.json.sig"));
    let issue = (&include_bytes!("fixtures/issues.json")[..], include_str!("fixtures/issues.json.sig"));
    let got = hooks.receive((&forgejo("push", "i-1", other.1), other.0), &roster(), now()).unwrap();
    assert!(matches!(got, Received::Ignored(Ignored::Unfollowed { .. })), "{got:?}");
    let got = hooks.receive((&forgejo("push", "i-2", gone.1), gone.0), &roster(), now()).unwrap();
    assert!(matches!(got, Received::Ignored(Ignored::Deleted { .. })), "{got:?}");
    let got = hooks.receive((&forgejo("issues", "i-3", issue.1), issue.0), &roster(), now()).unwrap();
    assert_eq!(got, Received::Ignored(Ignored::Event { written: "issues".to_owned() }));
}

#[test]
fn a_missing_header_and_an_oversize_body_are_typed() {
    let none = Headers::new();
    assert!(matches!(
        hooks().receive((&none, PUSH), &roster(), now()),
        Err(Refused::Missing { header: "X-Gitea-Signature" })
    ));
    let small = Hooks::new(codeberg(), Secret::new(SECRET), Limits { body: 8, ..Limits::DEFAULT });
    let got = small.receive((&forgejo("push", "f-9", PUSH_SIG), PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Oversize { limit: 8, .. })), "{got:?}");
}
