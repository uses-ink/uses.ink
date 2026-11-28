//! A GitLab delivery against recorded bodies and the secret token header.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_gitlab as authority_gitlab;

use authority_forge::{Headers, Host, Ignored, Limits, Received, Refused, Repo, Roster, Secret};
use authority_gitlab::Hooks;
use jiff::{SignedDuration, Timestamp};

const TOKEN: &str = "gitlab-hook-token";
const PUSH: &[u8] = include_bytes!("fixtures/push.json");
const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

fn hooks() -> Hooks {
    Hooks::new(Host::GITLAB, Secret::new(TOKEN), Limits::DEFAULT)
}

fn roster() -> Roster {
    [Repo::new(Host::GITLAB, "octo/notes/docs".parse().unwrap())].into_iter().collect()
}

fn headers(event: &str, id: &str, token: &str) -> Headers {
    Headers::new().with("X-Gitlab-Event", event).with("X-Gitlab-Event-UUID", id).with("X-Gitlab-Token", token)
}

#[test]
fn a_delivery_with_the_token_is_a_typed_push_in_a_subgroup() {
    let got = hooks().receive((&headers("Push Hook", "u-1", TOKEN), PUSH), &roster(), now()).unwrap();
    let Received::Push(push) = got else { panic!("{got:?}") };
    assert_eq!(push.repo.to_string(), "gitlab.com/octo/notes/docs");
    assert_eq!((push.branch.as_str(), push.revision.as_str()), ("main", SHA));
}

#[test]
fn a_wrong_token_of_any_length_is_refused() {
    for bad in ["", "x", "gitlab-hook-toke", "gitlab-hook-tokenn", "GITLAB-HOOK-TOKEN"] {
        let got = hooks().receive((&headers("Push Hook", "u-2", bad), PUSH), &roster(), now());
        assert!(matches!(got, Err(Refused::Signature)), "{bad}: {got:?}");
    }
    let got = hooks().receive((&Headers::new().with("X-Gitlab-Event", "Push Hook"), PUSH), &roster(), now());
    assert!(matches!(got, Err(Refused::Missing { header: "X-Gitlab-Token" })), "{got:?}");
}

#[test]
fn a_replayed_delivery_is_refused_and_the_older_header_works_too() {
    let hooks = hooks();
    let older =
        Headers::new().with("X-Gitlab-Event", "Push Hook").with("Idempotency-Key", "k-1").with("X-Gitlab-Token", TOKEN);
    hooks.receive((&older, PUSH), &roster(), now()).unwrap();
    let again = hooks.receive((&older, PUSH), &roster(), now() + SignedDuration::from_secs(2));
    assert!(matches!(again, Err(Refused::Replayed { .. })), "{again:?}");
}

#[test]
fn other_repositories_deletions_and_other_events_are_ignored() {
    let hooks = hooks();
    let other = &include_bytes!("fixtures/push-other.json")[..];
    let gone = &include_bytes!("fixtures/push-delete.json")[..];
    let tag = &include_bytes!("fixtures/tag-push.json")[..];
    let got = hooks.receive((&headers("Push Hook", "i-1", TOKEN), other), &roster(), now()).unwrap();
    assert!(matches!(got, Received::Ignored(Ignored::Unfollowed { .. })), "{got:?}");
    let got = hooks.receive((&headers("Push Hook", "i-2", TOKEN), gone), &roster(), now()).unwrap();
    assert!(matches!(got, Received::Ignored(Ignored::Deleted { .. })), "{got:?}");
    let got = hooks.receive((&headers("Tag Push Hook", "i-3", TOKEN), tag), &roster(), now()).unwrap();
    assert_eq!(got, Received::Ignored(Ignored::Event { written: "Tag Push Hook".to_owned() }));
}

#[test]
fn a_body_that_is_not_an_event_is_malformed_after_the_token_held() {
    let got = hooks().receive((&headers("Push Hook", "m-1", TOKEN), b"{}"), &roster(), now());
    let err = got.unwrap_err();
    assert!(matches!(err, Refused::Json(_)));
    assert_eq!(err.code(), "hook.payload.malformed");
    let big = Hooks::new(Host::GITLAB, Secret::new(TOKEN), Limits { body: 4, ..Limits::DEFAULT });
    assert!(matches!(
        big.receive((&headers("Push Hook", "m-2", TOKEN), PUSH), &roster(), now()),
        Err(Refused::Oversize { .. })
    ));
    assert!(!format!("{:?}", hooks()).contains(TOKEN));
}
