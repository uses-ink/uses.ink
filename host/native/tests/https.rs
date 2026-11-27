//! The `git` capability over https from a local TLS server whose certificate authority is made
//! here (decision 0072): the clone the https transport makes is the one the file transport makes,
//! and a certificate no root vouches for fails the fetch with its cause. No network is touched.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_native as host_native;

#[path = "support/origin.rs"]
mod origin;
#[path = "../../../https/tests/support/server.rs"]
mod server;

use std::fmt::Write;

use host_native::{Failed, Fetch};
use https::Https;
use model::git::{Depth, Network, Refused, Request};
use fixture::origin::Origin;
use origin::Fetching as _;
use server::{Authority, Mode, Server};

fn over(origin: &Origin, server: &Server, https: Https) -> Result<model::git::Cloned, Refused<Failed>> {
    let fetch = Fetch { network: Network::On, https, ..origin.fetch() };
    fetch.run(&Request::new(server.url().parse().unwrap()))
}

#[test]
fn a_clone_over_https_is_the_clone_the_file_transport_makes() {
    let origin = Origin::named("https-clone");
    let authority = Authority::new();
    let server = Server::start(&authority, &origin.root, Mode::Serve);
    let cloned = over(&origin, &server, Https::default().trusting(authority.root())).unwrap();
    let local = origin.fetch().run(&origin.request(Depth::Full)).unwrap();
    assert_eq!(cloned.commit, local.commit);
    assert_eq!(cloned.objects, local.objects);
    assert!(server.requests() >= 2, "the references, then the pack");

    let again = over(&origin, &server, Https::default().trusting(authority.root())).unwrap();
    assert_eq!(again.commit, cloned.commit, "a second fetch updates the clone it holds");
}

#[test]
fn a_certificate_no_root_vouches_for_fails_the_fetch_with_its_cause() {
    let origin = Origin::named("https-untrusted");
    let server = Server::start(&Authority::new(), &origin.root, Mode::Serve);
    let refused = over(&origin, &server, Https::default().trusting(Authority::new().root())).unwrap_err();
    assert!(matches!(refused, Refused::Failed(Failed::Fetch { .. })), "{refused:?}");
    let mut text = String::new();
    let mut next = std::error::Error::source(&refused);
    while let Some(cause) = next {
        write!(text, "{cause} {cause:?} | ").unwrap();
        next = cause.source();
    }
    assert!(text.contains("UnknownIssuer") || text.to_lowercase().contains("certificate"), "{text}");
    assert_eq!(server.requests(), 0);
}
