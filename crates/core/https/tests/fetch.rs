//! An API call over https against a forge on the loopback whose certificate authority is made here:
//! a name sent to the test's address, the certificate checked for the name, a redirect left alone,
//! a reply cut at its limit and a call that stalls ended by its timeout. No network is touched.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_https as https;

#[path = "support/forge.rs"]
mod forge;

use std::time::{Duration, Instant};

use forge::{Authority, Forge, Reply};
use https::{Call, Failed, Fetch, Https, Route, Verb};

const NAME: &str = "forge.test";

fn fetch(authority: &Authority, forge: &Forge) -> Fetch {
    let https = Https::default().trusting(authority.root());
    Fetch::new(&https, &[Route { host: NAME.to_owned(), address: forge.address() }]).unwrap()
}

fn call<'a>(path: &'a str, headers: &'a [(&'a str, &'a [u8])]) -> Call<'a> {
    Call { verb: Verb::Get, url: path, headers, body: b"", timeout: Duration::from_secs(5), limit: 1024 }
}

#[tokio::test]
async fn a_name_is_sent_to_its_route_and_the_answer_is_read_with_its_status() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("GET", "/api/ok", Reply::Status(200, b"{\"a\":1}".to_vec()));
    forge.reply("GET", "/api/gone", Reply::Status(404, b"{}".to_vec()));
    let fetch = fetch(&authority, &forge);
    let headers: [(&str, &[u8]); 2] = [("Authorization", b"Bearer s3cret"), ("Accept", b"application/json")];
    let url = format!("https://{NAME}/api/ok");
    let answer = fetch.send(call(&url, &headers)).await.unwrap();
    assert_eq!((answer.status, answer.body.as_slice()), (200, &b"{\"a\":1}"[..]));
    let seen = forge.seen();
    assert_eq!(seen[0].header("authorization"), Some("Bearer s3cret"));
    let gone = fetch.send(call(&format!("https://{NAME}/api/gone"), &[])).await.unwrap();
    assert_eq!(gone.status, 404, "a status is an answer, not a failure");
}

#[tokio::test]
async fn a_post_carries_its_body() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("POST", "/api/make", Reply::Status(201, b"{}".to_vec()));
    let fetch = fetch(&authority, &forge);
    let url = format!("https://{NAME}/api/make");
    let headers: [(&str, &[u8]); 1] = [("Content-Type", b"application/json")];
    let call = Call { verb: Verb::Post, body: b"{\"x\":true}", ..call(&url, &headers) };
    assert_eq!(fetch.send(call).await.unwrap().status, 201);
    assert_eq!(forge.seen()[0].body, b"{\"x\":true}");
}

#[tokio::test]
async fn a_redirect_is_the_answer_and_is_not_followed() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("GET", "/api/moved", Reply::Redirect(format!("https://{NAME}/api/ok")));
    forge.reply("GET", "/api/ok", Reply::Status(200, b"{}".to_vec()));
    let fetch = fetch(&authority, &forge);
    let answer = fetch.send(call(&format!("https://{NAME}/api/moved"), &[])).await.unwrap();
    assert_eq!(answer.status, 302);
    assert_eq!(forge.seen().len(), 1, "the redirect was not followed");
}

#[tokio::test]
async fn a_reply_is_cut_one_byte_past_its_limit() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("GET", "/api/big", Reply::Status(200, vec![b'x'; 5000]));
    let fetch = fetch(&authority, &forge);
    let answer = fetch.send(call(&format!("https://{NAME}/api/big"), &[])).await.unwrap();
    assert_eq!(answer.body.len(), 1025, "the limit and one byte more, which tells the caller it was cut");
}

#[tokio::test]
async fn a_call_that_stalls_ends_at_its_timeout() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("GET", "/api/slow", Reply::Stall);
    let fetch = fetch(&authority, &forge);
    let url = format!("https://{NAME}/api/slow");
    let started = Instant::now();
    let failed = fetch.send(Call { timeout: Duration::from_millis(300), ..call(&url, &[]) }).await.unwrap_err();
    assert!(matches!(failed, Failed::Send(_)), "{failed:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn a_certificate_the_roots_do_not_vouch_for_is_refused_and_so_is_http() {
    let authority = Authority::new();
    let forge = Forge::start(&authority, &[NAME]);
    forge.reply("GET", "/api/ok", Reply::Status(200, b"{}".to_vec()));
    // the same client without the test authority among its roots
    let untrusting =
        Fetch::new(&Https::default(), &[Route { host: NAME.to_owned(), address: forge.address() }]).unwrap();
    let url = format!("https://{NAME}/api/ok");
    assert!(matches!(untrusting.send(call(&url, &[])).await, Err(Failed::Send(_))));
    let fetch = fetch(&authority, &forge);
    let plain = format!("http://{NAME}/api/ok");
    assert!(matches!(fetch.send(call(&plain, &[])).await, Err(Failed::Send(_))), "https only");
    // a header that cannot be sent is named, and its value is not
    let bad: [(&str, &[u8]); 1] = [("X-Bad", b"line\nbreak")];
    let failed = fetch.send(call(&url, &bad)).await.unwrap_err();
    assert!(matches!(&failed, Failed::Header { name } if name == "X-Bad"), "{failed:?}");
    assert!(!format!("{:?}", call(&url, &bad)).contains("break"));
}
