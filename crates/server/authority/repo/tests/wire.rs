//! The real transport against a forge on the loopback whose certificate authority is made here: a
//! challenge read through https, a host that is not on the list never called, and a reply that is
//! too long refused. No network is touched.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_authority_repo as authority_repo;

#[path = "../../../../core/https/tests/support/forge.rs"]
mod forge;

use std::time::Duration;

use authority_forge::{Challenge, Fail, Host, Limits, Repo, Unproven};
use authority_forgejo::Instances;
use authority_repo::{Forges, Repos, Unwired, Wire};
use forge::{Authority, Forge, Reply};
use https::{Fetch, Https, Route};

const NAME: &str = "forge.test";

fn repos(authority: &Authority, forge: &Forge, limits: Limits) -> Repos<Wire> {
    let https = Https::default().trusting(authority.root());
    let fetch = Fetch::new(&https, &[Route { host: NAME.to_owned(), address: forge.address() }]).unwrap();
    let wire = Wire::new(fetch, vec![NAME.parse().unwrap()], "uses-ink-test".to_owned());
    let instances = Instances::read("[[instance]]\nname = \"Test\"\nhost = \"forge.test\"\nprimary = true\n").unwrap();
    Repos::new(Forges { forgejo: instances, gitlab: vec![Host::GITLAB], limits }, wire)
}

#[tokio::test]
async fn a_challenge_is_read_through_https_and_a_stranger_host_is_never_called() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    let challenge = Challenge::from_bytes([9; 16]);
    forge.reply(
        "GET",
        "/api/v1/repos/a/b/raw/.well-known/uses-ink",
        Reply::Status(200, format!("{challenge}\n").into()),
    );
    let repos = repos(&authority, &forge, Limits::DEFAULT);
    let repo: Repo = format!("{NAME}/a/b").parse().unwrap();
    assert_eq!(repos.read(&repo, &challenge).await.unwrap().repo, repo);
    assert_eq!(forge.seen()[0].header("user-agent"), Some("uses-ink-test"));
    let wrong = Challenge::from_bytes([8; 16]);
    assert!(matches!(repos.read(&repo, &wrong).await, Err(Unproven::Wrong)));
    let stranger: Repo = "gitlab.example/a/b".parse().unwrap();
    // gitlab.example is on no list: the forge never hears of it
    let failed = repos.read(&stranger, &challenge).await.unwrap_err();
    assert!(matches!(failed, Unproven::Unlisted { .. }), "{failed:?}");
    assert_eq!(forge.seen().len(), 2);
}

#[tokio::test]
async fn a_reply_over_the_limit_is_refused_and_a_redirect_is_a_status() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("GET", "/api/v1/repos/a/b/raw/.well-known/uses-ink", Reply::Status(200, vec![b'x'; 4096]));
    forge.reply("GET", "/api/v1/repos/c/d/raw/.well-known/uses-ink", Reply::Redirect("https://elsewhere.test/".into()));
    let limits = Limits { reply: 1024, timeout: Duration::from_secs(5), ..Limits::DEFAULT };
    let repos = repos(&authority, &forge, limits);
    let challenge = Challenge::from_bytes([9; 16]);
    let big: Repo = format!("{NAME}/a/b").parse().unwrap();
    let failed = repos.read(&big, &challenge).await.unwrap_err();
    assert!(matches!(failed, Unproven::Call(Fail::Oversize { limit: 1024, .. })), "{failed:?}");
    let moved: Repo = format!("{NAME}/c/d").parse().unwrap();
    let failed = repos.read(&moved, &challenge).await.unwrap_err();
    assert!(matches!(failed, Unproven::Call(Fail::Status { status: 302 })), "{failed:?}");
}

#[tokio::test]
async fn the_wire_names_a_host_it_was_not_given() {
    use authority_forge::{Client, Method, Request, Transport};
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    let https = Https::default().trusting(authority.root());
    let fetch = Fetch::new(&https, &[]).unwrap();
    let wire = Client::new(Wire::new(fetch, vec![NAME.parse().unwrap()], "t".to_owned()), Limits::DEFAULT);
    let request = |url: &str| Request::new(Method::Get, url.parse().unwrap());
    let failed = wire.transport().send(request("https://other.test/x")).await.unwrap_err();
    assert!(matches!(failed, Unwired::Unlisted { ref host } if host == "other.test"), "{failed:?}");
    let failed = wire.transport().send(request("http://forge.test/x")).await.unwrap_err();
    assert!(matches!(failed, Unwired::Scheme { .. }), "{failed:?}");
    assert!(forge.seen().is_empty());
}
