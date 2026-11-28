//! The App against recorded replies: its JWT, ownership by installation, and commit statuses with
//! an installation token that is exchanged for each call and never kept.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_github as authority_github;

use authority_forge::{
    Client, Fail, Fake, Faked, Host, Limits, Method, Repo, Response, Secret, Segment, State, Status,
};
use authority_github::{App, AppId, Armor, Error, Grant, InstallationId, Jwt, Key, KeyError, Signer, Target};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jiff::Timestamp;
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey};

const PEM: &str = include_str!("fixtures/app-pkcs1.txt");
const PKCS8: &str = include_str!("fixtures/app-pkcs8.txt");
const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";
const INSTALLATION: &str = "https://api.github.com/repos/octo/docs/installation";
const TOKENS: &str = "https://api.github.com/app/installations/123/access_tokens";
const STATUSES: &str = "https://api.github.com/repos/octo/docs/statuses/6dcb09b5b57875f334f61aebed695e2e4193db5e";

type Tested = App<Key, Fake>;

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

fn app_with(limits: Limits) -> Tested {
    App::new(AppId(987), Key::read(&Secret::new(PEM)).unwrap(), Client::new(Fake::new(), limits))
}

fn app() -> Tested {
    app_with(Limits::DEFAULT)
}

fn repo() -> Repo {
    Repo::new(Host::GITHUB, "octo/docs".parse().unwrap())
}

fn octo() -> Segment {
    "octo".parse().unwrap()
}

fn grant() -> Grant {
    Grant { installation: InstallationId(123), repo: repo() }
}

fn target() -> Target {
    Target { grant: grant(), revision: SHA.parse().unwrap() }
}

fn reply(app: &Tested, method: Method, url: &str, said: (u16, &[u8])) {
    app.client().transport().reply(method, url, Response::new(said.0, said.1));
}

#[test]
fn the_jwt_is_rs256_with_a_backdated_short_lived_claim() {
    let key = Key::read(&Secret::new(PEM)).unwrap();
    let jwt = Jwt::mint(&key, AppId(987), now()).unwrap();
    let text = jwt.text().unwrap();
    let parts: Vec<&str> = text.split('.').collect();
    assert_eq!(parts.len(), 3);
    let part =
        |at: usize| serde_json::from_slice::<serde_json::Value>(&URL_SAFE_NO_PAD.decode(parts[at]).unwrap()).unwrap();
    assert_eq!(part(0), serde_json::json!({ "alg": "RS256", "typ": "JWT" }));
    let claims = part(1);
    assert_eq!(claims["iss"], 987);
    assert_eq!(claims["iat"], 1_800_000_000 - 60);
    assert_eq!(claims["exp"], 1_800_000_000 + 9 * 60);
    let signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    let message = format!("{}.{}", parts[0], parts[1]);
    UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, key.public()).verify(message.as_bytes(), &signature).unwrap();
    assert_eq!(format!("{jwt:?}"), "Secret(<redacted>)");
}

#[test]
fn a_key_reads_as_pkcs1_or_pkcs8_and_other_text_is_refused_with_its_cause() {
    assert!(Key::read(&Secret::new(PKCS8)).is_ok());
    assert!(matches!(Key::read(&Secret::new("")), Err(KeyError::Frame)));
    assert!(matches!(
        Key::read(&Secret::new("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----")),
        Err(KeyError::Label { .. })
    ));
    assert!(matches!(
        Key::read(&Secret::new("-----BEGIN RSA PRIVATE KEY-----\n!!!!\n-----END RSA PRIVATE KEY-----")),
        Err(KeyError::Base64(_))
    ));
    assert!(matches!(
        Key::read(&Secret::new("-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----")),
        Err(KeyError::Rejected(_))
    ));
    assert!(matches!(Key::read(&Secret::new("-----BEGIN RSA PRIVATE KEY-----\nAAAA")), Err(KeyError::Frame)));
    assert_eq!(format!("{:?}", Key::read(&Secret::new(PEM)).unwrap()), "Key(<redacted>)");
}

#[test]
fn a_label_that_is_not_a_key_is_named_and_never_echoed() {
    let secret = "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC";
    let pasted = format!("-----BEGIN {secret}-----\nAAAA\n-----END {secret}-----");
    let refused = Key::read(&Secret::new(pasted)).unwrap_err();
    assert!(matches!(refused, KeyError::Label { armor: Armor::Other { length } } if length == secret.len()));
    assert!(!format!("{refused} {refused:?}").contains(secret));
    let known = Key::read(&Secret::new("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----")).unwrap_err();
    assert!(matches!(known, KeyError::Label { armor: Armor::Certificate }));
}

#[test]
fn the_installation_that_covers_the_repo_and_matches_the_account_proves_it() {
    let app = app();
    reply(&app, Method::Get, INSTALLATION, (200, include_bytes!("fixtures/installation.json")));
    let covered = pollster::block_on(app.owns((&repo(), &octo()), now())).unwrap();
    assert_eq!((covered.installation, covered.account.as_str()), (InstallationId(123), "octo"));
    let sent = app.client().transport().taken();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].url.as_str(), INSTALLATION);
    assert_eq!(sent[0].bearer.as_ref().unwrap().text().unwrap().split('.').count(), 3);
}

#[test]
fn a_repo_without_the_app_or_with_another_account_or_suspended_is_not_proven() {
    let own = |status: u16, body: &[u8], claimed: &str| {
        let app = app();
        reply(&app, Method::Get, INSTALLATION, (status, body));
        pollster::block_on(app.owns((&repo(), &claimed.parse().unwrap()), now())).unwrap_err()
    };
    assert!(matches!(own(404, include_bytes!("fixtures/not-found.json"), "octo"), Error::NotInstalled));
    assert!(matches!(own(200, include_bytes!("fixtures/installation-suspended.json"), "octo"), Error::Suspended));
    assert!(matches!(own(200, include_bytes!("fixtures/installation-other.json"), "octo"), Error::Account { .. }));
    assert!(matches!(own(200, include_bytes!("fixtures/installation.json"), "mallory"), Error::Account { .. }));
    assert!(matches!(own(500, b"", "octo"), Error::Call(Fail::Status { status: 500 })));
    assert!(matches!(own(200, b"[]", "octo"), Error::Call(Fail::Json(_))));
    let err = own(404, b"", "octo");
    assert_eq!(err.code(), "repo.app.missing");
}

#[test]
fn another_forge_is_not_github() {
    let app = app();
    let elsewhere = Repo::new("codeberg.org".parse().unwrap(), "octo/docs".parse().unwrap());
    let err = pollster::block_on(app.owns((&elsewhere, &octo()), now())).unwrap_err();
    assert!(matches!(err, Error::Unlisted { .. }));
    assert!(app.client().transport().taken().is_empty());
}

#[test]
fn a_status_exchanges_a_token_for_each_call_and_keeps_none() {
    let app = app();
    reply(&app, Method::Post, TOKENS, (201, include_bytes!("fixtures/token-1.json")));
    app.client().transport().then(
        Method::Post,
        TOKENS,
        Response::new(201, &include_bytes!("fixtures/token-2.json")[..]),
    );
    reply(&app, Method::Post, STATUSES, (201, include_bytes!("fixtures/status.json")));
    let status = Status::new(State::Success).note("built").link("https://preview.uses.ink/".parse().unwrap());
    pollster::block_on(app.status(&target(), &status, now())).unwrap();
    pollster::block_on(app.status(&target(), &status, now())).unwrap();
    let sent = app.client().transport().taken();
    let urls: Vec<&str> = sent.iter().map(|request| request.url.as_str()).collect();
    assert_eq!(urls, [TOKENS, STATUSES, TOKENS, STATUSES]);
    let first = sent[1].bearer.as_ref().unwrap().text().unwrap();
    let second = sent[3].bearer.as_ref().unwrap().text().unwrap();
    assert_eq!(
        (first, second),
        ("ghs_synthetic0000000000000000000000000001", "ghs_synthetic0000000000000000000000000002")
    );
    let body: serde_json::Value = serde_json::from_slice(&sent[1].body).unwrap();
    assert_eq!(body["state"], "success");
    assert_eq!(body["context"], "uses.ink");
    let exchange: serde_json::Value = serde_json::from_slice(&sent[0].body).unwrap();
    assert_eq!(exchange["repositories"], serde_json::json!(["docs"]));
    assert_eq!(
        exchange["permissions"],
        serde_json::json!({ "contents": "read", "metadata": "read", "statuses": "write" })
    );
    let shown = format!("{sent:?} {app:?}");
    assert!(!shown.contains("ghs_"), "{shown}");
    assert!(!shown.contains("PRIVATE KEY"), "{shown}");
}

#[test]
fn a_token_is_never_in_an_error_or_a_log_line() {
    let app = app();
    reply(&app, Method::Post, TOKENS, (201, include_bytes!("fixtures/token-1.json")));
    reply(&app, Method::Post, STATUSES, (403, br#"{"message":"Resource not accessible by integration"}"#));
    let err = pollster::block_on(app.status(&target(), &Status::new(State::Failure), now())).unwrap_err();
    assert!(matches!(err, Error::Call(Fail::Status { status: 403 })));
    assert_eq!(err.code(), "forge.call.refused");
    assert!(!format!("{err:?} {err}").contains("ghs_"));
}

#[test]
fn a_token_for_a_build_comes_back_as_a_secret_and_the_app_keeps_nothing() {
    let app = app();
    reply(&app, Method::Post, TOKENS, (201, include_bytes!("fixtures/token-1.json")));
    let token = pollster::block_on(app.token(&grant(), now())).unwrap();
    assert_eq!(format!("{token:?}"), "Secret(<redacted>)");
    assert!(token.text().unwrap().starts_with("ghs_"));
    assert!(!format!("{app:?}").contains("ghs_"));
}

#[test]
fn an_unreachable_forge_and_an_oversize_reply_are_typed() {
    let app = app();
    app.client().transport().fail(true);
    let err = pollster::block_on(app.owns((&repo(), &octo()), now())).unwrap_err();
    assert!(matches!(err, Error::Call(Fail::Transport(Faked::Down))));
    let tight = app_with(Limits { reply: 8, ..Limits::DEFAULT });
    reply(&tight, Method::Get, INSTALLATION, (200, include_bytes!("fixtures/installation.json")));
    let err = pollster::block_on(tight.owns((&repo(), &octo()), now())).unwrap_err();
    assert!(matches!(err, Error::Call(Fail::Oversize { limit: 8, .. })));
}

#[test]
fn a_signer_is_a_port() {
    struct Refuses;
    #[derive(Debug)]
    struct Declined;
    impl std::fmt::Display for Declined {
        fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            out.write_str("declined")
        }
    }
    impl std::error::Error for Declined {}
    impl Signer for Refuses {
        type Error = Declined;
        fn sign(&self, _: &[u8]) -> Result<Vec<u8>, Declined> {
            Err(Declined)
        }
    }
    let app = App::new(AppId(1), Refuses, Client::new(Fake::new(), Limits::DEFAULT));
    let err = pollster::block_on(app.owns((&repo(), &octo()), now())).unwrap_err();
    assert!(matches!(err, Error::Sign(_)));
    assert_eq!(err.code(), "app.sign.failed");
}
