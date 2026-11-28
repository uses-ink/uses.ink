//! The repo challenge on GitHub, read with no credential, and the repository a delivery names.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_github as authority_github;

use authority_forge::{Challenge, Client, Fake, Host, Limits, Method, Refused, Repo, Response, Unproven};
use authority_github::{Hooks, Open};

const CONTENTS: &str = "https://api.github.com/repos/octo/docs/contents/.well-known/uses-ink";

fn open() -> Open<Fake> {
    Open::new(Client::new(Fake::new(), Limits::DEFAULT))
}

fn repo(host: Host) -> Repo {
    Repo::new(host, "octo/docs".parse().unwrap())
}

#[test]
fn the_challenge_file_proves_write_access_with_no_credential() {
    let open = open();
    let fake = open.client().transport();
    fake.reply(Method::Get, CONTENTS, Response::new(200, "uses-ink-repo=07070707070707070707070707070707\n"));
    let proven = pollster::block_on(open.prove(&repo(Host::GITHUB), &Challenge::from_bytes([7; 16]))).unwrap();
    assert_eq!(proven.owner.as_str(), "octo");
    let sent = fake.taken();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].bearer.is_none(), "the proof reads a public file and sends no credential");
    assert!(sent[0].headers.iter().any(|(name, value)| *name == "Accept" && value.contains("raw")));
}

#[test]
fn a_missing_wrong_or_failed_read_is_typed() {
    let open = open();
    let fake = open.client().transport();
    let ask = || pollster::block_on(open.prove(&repo(Host::GITHUB), &Challenge::from_bytes([7; 16])));
    fake.reply(Method::Get, CONTENTS, Response::new(404, "{}"));
    assert!(matches!(ask(), Err(Unproven::Absent)));
    fake.reply(Method::Get, CONTENTS, Response::new(200, "uses-ink-repo=0\n"));
    assert!(matches!(ask(), Err(Unproven::Wrong)));
    fake.reply(Method::Get, CONTENTS, Response::new(500, "{}"));
    assert!(matches!(ask(), Err(Unproven::Call(_))));
    fake.fail(true);
    assert!(matches!(ask(), Err(Unproven::Call(_))));
}

#[test]
fn another_host_is_unlisted_and_sends_nothing() {
    let open = open();
    let got = pollster::block_on(open.prove(&repo("gitlab.com".parse().unwrap()), &Challenge::from_bytes([7; 16])));
    assert!(matches!(got, Err(Unproven::Unlisted { .. })));
    assert!(open.client().transport().taken().is_empty());
}

#[test]
fn a_delivery_names_its_repository_before_it_is_trusted() {
    let named = Hooks::named(br#"{"repository":{"full_name":"Octo/Docs"}}"#).unwrap();
    assert_eq!(named.to_string(), "github.com/octo/docs");
    assert!(matches!(Hooks::named(b"{}"), Err(Refused::Json(_))));
    assert!(matches!(Hooks::named(br#"{"repository":{"full_name":"solo"}}"#), Err(Refused::Name(_))));
}
