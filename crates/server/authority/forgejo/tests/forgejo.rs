//! The repo challenge and commit statuses against recorded replies, and the bundled instances.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_forgejo as authority_forgejo;

use authority_forge::{
    Challenge, Client, Fail, Fake, Host, Limits, Method, Pinned, Repo, Response, Secret, State, Status, Unproven,
};
use authority_forgejo::{Forgejo, Instances, Unreadable};

const RAW: &str = "https://codeberg.org/api/v1/repos/octo/docs/raw/.well-known/uses-ink";
const STATUSES: &str = "https://codeberg.org/api/v1/repos/octo/docs/statuses/6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn forgejo() -> Forgejo<Fake> {
    Forgejo::new(Instances::bundled().unwrap(), Client::new(Fake::new(), Limits::DEFAULT))
}

fn repo(host: &str) -> Repo {
    Repo::new(host.parse().unwrap(), "octo/docs".parse().unwrap())
}

fn challenge() -> Challenge {
    Challenge::from_bytes([7; 16])
}

#[test]
fn codeberg_is_the_bundled_primary_instance() {
    let instances = Instances::bundled().unwrap();
    let primary = instances.primary().unwrap();
    assert_eq!(
        (primary.name.as_str(), primary.host.as_str(), primary.preregistered),
        ("Codeberg", "codeberg.org", true)
    );
    assert!(instances.find(&"codeberg.org".parse::<Host>().unwrap()).is_some());
    assert!(instances.find(&"example.org".parse::<Host>().unwrap()).is_none());
}

#[test]
fn an_operator_list_needs_one_primary_and_no_host_twice() {
    let one = "[[instance]]\nname = \"A\"\nhost = \"a.test\"\nprimary = true\n";
    assert_eq!(Instances::read(one).unwrap().all().len(), 1);
    assert!(matches!(Instances::read("instance = []"), Err(Unreadable::Primary { found: 0 })));
    assert!(matches!(Instances::read(&format!("{one}{one}")), Err(Unreadable::Primary { found: 2 })));
    let twice = format!("{one}[[instance]]\nname = \"B\"\nhost = \"A.test\"\n");
    assert!(matches!(Instances::read(&twice), Err(Unreadable::Twice { .. })));
    assert!(matches!(Instances::read("not toml ["), Err(Unreadable::Toml(_))));
    assert!(matches!(
        Instances::read("[[instance]]\nname = \"A\"\nhost = \"a b\"\nprimary = true\n"),
        Err(Unreadable::Toml(_))
    ));
}

#[test]
fn the_challenge_file_proves_write_access() {
    let forgejo = forgejo();
    let fake = forgejo.client().transport();
    fake.reply(Method::Get, RAW, Response::new(200, &include_bytes!("fixtures/challenge.txt")[..]));
    let proven = pollster::block_on(forgejo.prove(&repo("codeberg.org"), &challenge())).unwrap();
    assert_eq!((proven.owner.as_str(), proven.repo.to_string().as_str()), ("octo", "codeberg.org/octo/docs"));
    assert!(fake.taken()[0].bearer.is_none(), "the proof reads with no credential");
}

#[test]
fn a_missing_or_wrong_challenge_or_an_unlisted_host_is_not_a_proof() {
    let forgejo = forgejo();
    let fake = forgejo.client().transport();
    fake.reply(Method::Get, RAW, Response::new(404, &include_bytes!("fixtures/not-found.json")[..]));
    let got = pollster::block_on(forgejo.prove(&repo("codeberg.org"), &challenge()));
    assert!(matches!(got, Err(Unproven::Absent)), "{got:?}");
    fake.reply(Method::Get, RAW, Response::new(200, &include_bytes!("fixtures/challenge-wrong.txt")[..]));
    let got = pollster::block_on(forgejo.prove(&repo("codeberg.org"), &challenge()));
    assert!(matches!(got, Err(Unproven::Wrong)), "{got:?}");
    fake.reply(Method::Get, RAW, Response::new(500, ""));
    let got = pollster::block_on(forgejo.prove(&repo("codeberg.org"), &challenge()));
    assert!(matches!(got, Err(Unproven::Call(Fail::Status { status: 500 }))), "{got:?}");
    let before = fake.taken().len();
    let got = pollster::block_on(forgejo.prove(&repo("evil.test"), &challenge()));
    assert!(matches!(&got, Err(Unproven::Unlisted { host }) if host.as_str() == "evil.test"), "{got:?}");
    assert_eq!(before, 3);
    assert!(fake.taken().is_empty(), "an unlisted host is never called");
}

#[test]
fn a_status_moves_its_token_into_the_request() {
    let forgejo = forgejo();
    let fake = forgejo.client().transport();
    fake.reply(Method::Post, STATUSES, Response::new(201, &include_bytes!("fixtures/status.json")[..]));
    let at = Pinned::new(repo("codeberg.org"), "6dcb09b5b57875f334f61aebed695e2e4193db5e".parse().unwrap());
    pollster::block_on(forgejo.status(Secret::new("cb-synthetic-token"), &at, &Status::new(State::Pending))).unwrap();
    let sent = fake.taken();
    assert_eq!(sent[0].bearer.as_ref().unwrap().text(), Some("cb-synthetic-token"));
    let body: serde_json::Value = serde_json::from_slice(&sent[0].body).unwrap();
    assert_eq!(body["state"], "pending");
    assert!(!format!("{sent:?} {forgejo:?}").contains("cb-synthetic-token"));
    fake.reply(Method::Post, STATUSES, Response::new(403, ""));
    let err = pollster::block_on(forgejo.status(Secret::new("t"), &at, &Status::new(State::Error))).unwrap_err();
    assert!(matches!(err, Fail::Status { status: 403 }));
}
