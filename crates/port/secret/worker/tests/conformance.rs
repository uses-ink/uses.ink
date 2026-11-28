//! The adapter against the port's conformance suite and its own rotation case.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_secret_worker as secret_worker;

use secret_worker::{Rotated, Worker};

secret_conformance::suite!(|| (Worker::ephemeral().unwrap(), ()));

#[test]
fn keeps_what_the_old_key_sealed_readable_for_a_rotation() {
    pollster::block_on(Rotated::check());
}

#[test]
fn refuses_a_key_that_is_not_64_hexadecimal_digits_without_showing_it() {
    let error = Worker::from_hex("not a key", Some("SEALING_KEY")).unwrap_err();
    assert_eq!(error.to_string(), "the secret binding `SEALING_KEY` is not a key");
    assert!(!format!("{error:?}").contains("not a key"), "the text of the key is in the error");
}
