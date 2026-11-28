//! The nonce store's suite against the object's logic, and the boot rule the suite does not cover.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_edge_worker as edge_worker;

use edge_api::{Claimed, Claiming, NonceCase, Nonces};
use edge_worker::{Answer, Serving, Wire};
use sig::{KeyId, Nonce};

#[test]
fn the_object_passes_every_case() {
    for case in NonceCase::ALL {
        pollster::block_on(case.run(&Serving::new(0, NonceCase::WINDOW), 1_000));
    }
}

#[test]
fn a_rebuilt_object_refuses_what_was_created_before_it_was_built() {
    let serving = Serving::new(5_000, 60);
    let claim = |created: u64, now: u64| Claiming {
        key: KeyId::new([9; 16]),
        nonce: Nonce::new([9; 16]),
        until: created + 60,
        now,
        most: 4,
    };
    assert_eq!(pollster::block_on(serving.claim(claim(5_010, 5_020))).unwrap(), Claimed::Quiet { seconds: 40 });
    assert_eq!(pollster::block_on(serving.claim(claim(5_060, 5_061))).unwrap(), Claimed::Fresh);
}

#[test]
fn the_wire_round_trips_and_a_stranger_body_is_refused() {
    let serving = Serving::new(0, 60);
    let wire = Wire { key: [1; 16], nonce: [2; 16], until: 2_060, now: 2_000, most: 3 };
    let body = serde_json::to_vec(&wire).unwrap();
    let first: Answer = serde_json::from_slice(&serving.handle(&body).unwrap()).unwrap();
    let second: Answer = serde_json::from_slice(&serving.handle(&body).unwrap()).unwrap();
    assert_eq!((first, second), (Answer::Fresh, Answer::Replay));
    let refused = serving.handle(b"{\"key\":1}").unwrap_err();
    assert_eq!(refused.to_string(), "the body is not a nonce claim");
}
