//! The repo challenge and commit statuses against recorded replies.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_gitlab as authority_gitlab;

use authority_forge::{
    Challenge, Client, Fail, Fake, Host, Limits, Method, Pinned, Repo, Response, Secret, State, Status, Unproven,
};
use authority_gitlab::Gitlab;

const PROJECT: &str = "https://gitlab.com/api/v4/projects/octo%2Fnotes%2Fdocs";
const FILE: &str =
    "https://gitlab.com/api/v4/projects/octo%2Fnotes%2Fdocs/repository/files/.well-known%2Fuses-ink/raw?ref=main";
const STATUSES: &str =
    "https://gitlab.com/api/v4/projects/octo%2Fnotes%2Fdocs/statuses/6dcb09b5b57875f334f61aebed695e2e4193db5e";

fn gitlab() -> Gitlab<Fake> {
    Gitlab::hosted(Client::new(Fake::new(), Limits::DEFAULT))
}

fn repo(host: Host) -> Repo {
    Repo::new(host, "octo/notes/docs".parse().unwrap())
}

fn challenge() -> Challenge {
    Challenge::from_bytes([7; 16])
}

fn serve(gitlab: &Gitlab<Fake>, method: Method, url: &str, said: (u16, &[u8])) {
    gitlab.client().transport().reply(method, url, Response::new(said.0, said.1));
}

#[test]
fn the_challenge_file_on_the_default_branch_proves_write_access() {
    let gitlab = gitlab();
    serve(&gitlab, Method::Get, PROJECT, (200, include_bytes!("fixtures/project.json")));
    serve(&gitlab, Method::Get, FILE, (200, include_bytes!("fixtures/challenge.txt")));
    let proven = pollster::block_on(gitlab.prove(&repo(Host::GITLAB), &challenge())).unwrap();
    assert_eq!((proven.owner.as_str(), proven.repo.to_string().as_str()), ("octo", "gitlab.com/octo/notes/docs"));
    let sent = gitlab.client().transport().taken();
    assert_eq!(sent.iter().map(|request| request.url.as_str()).collect::<Vec<_>>(), [PROJECT, FILE]);
    assert!(sent.iter().all(|request| request.bearer.is_none()));
}

#[test]
fn a_missing_project_file_or_branch_and_a_wrong_challenge_are_not_a_proof() {
    let gitlab = gitlab();
    serve(&gitlab, Method::Get, PROJECT, (404, include_bytes!("fixtures/not-found.json")));
    assert!(matches!(pollster::block_on(gitlab.prove(&repo(Host::GITLAB), &challenge())), Err(Unproven::Absent)));
    serve(&gitlab, Method::Get, PROJECT, (200, include_bytes!("fixtures/project-empty.json")));
    assert!(matches!(pollster::block_on(gitlab.prove(&repo(Host::GITLAB), &challenge())), Err(Unproven::Absent)));
    serve(&gitlab, Method::Get, PROJECT, (200, include_bytes!("fixtures/project.json")));
    serve(&gitlab, Method::Get, FILE, (404, include_bytes!("fixtures/not-found.json")));
    assert!(matches!(pollster::block_on(gitlab.prove(&repo(Host::GITLAB), &challenge())), Err(Unproven::Absent)));
    serve(&gitlab, Method::Get, FILE, (200, include_bytes!("fixtures/challenge-wrong.txt")));
    assert!(matches!(pollster::block_on(gitlab.prove(&repo(Host::GITLAB), &challenge())), Err(Unproven::Wrong)));
    serve(&gitlab, Method::Get, FILE, (502, b""));
    let got = pollster::block_on(gitlab.prove(&repo(Host::GITLAB), &challenge()));
    assert!(matches!(got, Err(Unproven::Call(Fail::Status { status: 502 }))), "{got:?}");
}

#[test]
fn a_host_that_is_not_listed_is_never_called() {
    let gitlab = gitlab();
    let got = pollster::block_on(gitlab.prove(&repo("gitlab.evil.test".parse().unwrap()), &challenge()));
    assert!(matches!(got, Err(Unproven::Unlisted { .. })), "{got:?}");
    assert!(gitlab.client().transport().taken().is_empty());
    assert_eq!(got.unwrap_err().code(), "repo.host.unlisted");
}

#[test]
fn a_status_uses_gitlabs_states_and_moves_its_token_into_the_request() {
    let gitlab = gitlab();
    serve(&gitlab, Method::Post, STATUSES, (201, include_bytes!("fixtures/status.json")));
    let at = Pinned::new(repo(Host::GITLAB), "6dcb09b5b57875f334f61aebed695e2e4193db5e".parse().unwrap());
    pollster::block_on(gitlab.status(
        Secret::new("glpat-synthetic"),
        &at,
        &Status::new(State::Failure).note("did not build"),
    ))
    .unwrap();
    let sent = gitlab.client().transport().taken();
    assert_eq!(sent[0].bearer.as_ref().unwrap().text(), Some("glpat-synthetic"));
    let body: serde_json::Value = serde_json::from_slice(&sent[0].body).unwrap();
    assert_eq!((body["state"].as_str(), body["name"].as_str()), (Some("failed"), Some("uses.ink")));
    assert!(!format!("{sent:?} {gitlab:?}").contains("glpat-synthetic"));
    serve(&gitlab, Method::Post, STATUSES, (401, b""));
    let err = pollster::block_on(gitlab.status(Secret::new("t"), &at, &Status::new(State::Success))).unwrap_err();
    assert!(matches!(err, Fail::Status { status: 401 }));
}
