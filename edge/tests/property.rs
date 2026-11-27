//! Properties: whatever the bytes, the edge answers, never panics, and always hardens.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]
extern crate uses_ink_edge as edge;

mod support;

use std::sync::LazyLock;

use edge::{Name, Outcome, Request, Scheme};
use proptest::prelude::*;
use support::{Site, answer};

const STATUSES: [u16; 14] = [200, 206, 301, 304, 400, 404, 405, 408, 414, 416, 431, 500, 503, 504];

/// A path built from the pieces that matter: dots, escapes, slashes, real routes.
fn target() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("..".to_owned()),
        Just(".".to_owned()),
        Just("%2e".to_owned()),
        Just("%2f".to_owned()),
        Just("%252e".to_owned()),
        Just("%00".to_owned()),
        Just("%".to_owned()),
        Just("tea".to_owned()),
        Just("clip.png".to_owned()),
        Just("old".to_owned()),
        Just(String::new()),
        "[ -~]{0,6}",
        any::<String>(),
    ];
    prop_oneof![
        prop::collection::vec(piece, 0..6).prop_map(|pieces| format!("/{}", pieces.join("/"))),
        any::<String>(),
        "[ -~]{0,40}",
    ]
}

fn header() -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
    let name = prop_oneof![
        Just(b"host".to_vec()),
        Just(b"range".to_vec()),
        Just(b"if-range".to_vec()),
        Just(b"if-none-match".to_vec()),
        Just(b"accept-encoding".to_vec()),
        Just(b"content-length".to_vec()),
        Just(b"transfer-encoding".to_vec()),
        prop::collection::vec(any::<u8>(), 0..8),
    ];
    let value = prop_oneof![
        Just(b"notes.example".to_vec()),
        Just(b"bytes=0-9".to_vec()),
        Just(b"bytes=-1".to_vec()),
        Just(b"*".to_vec()),
        Just(b"gzip, br;q=0.5".to_vec()),
        prop::collection::vec(any::<u8>(), 0..40),
    ];
    (name, value)
}

static EDGE: LazyLock<edge::Edge<support::Hosts, support::Store>> = LazyLock::new(|| Site::new().edge());

proptest! {
    // Seeded, so a failure reproduces; proptest prints the failing input.
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x1a4b_2c3d),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_request_gets_a_hardened_answer(
        method in prop_oneof![Just("GET".to_owned()), Just("HEAD".to_owned()), "[A-Za-z]{0,8}"],
        target in target(),
        headers in prop::collection::vec(header(), 0..8),
        host in prop::option::of(prop_oneof![Just(b"notes.example".to_vec()), prop::collection::vec(any::<u8>(), 0..30)]),
    ) {
        let mut request = Request::new(&method, &target, Scheme::Plain);
        if let Some(host) = &host {
            request.extend([(b"host".as_slice(), host.as_slice())]);
        }
        request.extend(headers.iter().map(|(name, value)| (name.as_slice(), value.as_slice())));
        let (got, outcome) = answer(&EDGE, &request);
        prop_assert!(STATUSES.contains(&got.status), "status {}", got.status);
        prop_assert_eq!(got.field("x-content-type-options"), Some("nosniff"));
        prop_assert!(got.field("content-security-policy").is_some());
        prop_assert!(got.field(Name::ContentLength.name()).is_some() || got.status == 304 || got.status == 301);
        if let Some(length) = got.field("content-length").and_then(|text| text.parse::<usize>().ok())
            && method != "HEAD"
            && got.status != 304
        {
            prop_assert_eq!(got.body.len(), length);
        }
        if let Outcome::Refused(refusal) = &outcome {
            prop_assert_eq!(refusal.status().code(), got.status);
        }
        if let Outcome::Moved { to, .. } = &outcome {
            let location = got.field("location").unwrap();
            prop_assert!(location.starts_with('/') && !location.starts_with("//"), "{} for {}", location, to);
            prop_assert!(location.is_ascii());
        }
    }

    #[test]
    fn a_target_that_names_a_route_is_normalised_never_climbing(target in target()) {
        let limits = edge::Limits::default();
        if let Ok(parsed) = edge::Target::parse(&target, &limits) {
            let path = parsed.path();
            prop_assert!(path.starts_with('/'));
            prop_assert!(!path.contains("//") && !path.contains('\\') && !path.contains('\0') && !path.contains('%'));
            prop_assert!(path.split('/').all(|segment| segment != ".." && segment != "."));
            prop_assert!(path == "/" || !path.ends_with('/'));
        }
    }

    #[test]
    fn a_host_is_never_something_a_path_join_could_climb_with(host in prop::collection::vec(any::<u8>(), 0..300)) {
        if let Ok(authority) = edge::Authority::try_from(host.as_slice()) {
            let text = authority.hostname.as_str();
            prop_assert!(!text.contains('/') && !text.contains('\\') && !text.contains('\0'));
            if let Some(name) = authority.hostname.name() {
                prop_assert!(name.split('.').all(|label| !label.is_empty() && label != ".."));
                prop_assert!(name.len() <= edge::Hostname::LIMIT);
            }
        }
    }
}
