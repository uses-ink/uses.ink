//! The nonce store's suite against the memory store.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_edge_api as edge_api;

use edge_api::{NonceCase, Remembered};

#[test]
fn memory_passes_every_case() {
    for case in NonceCase::ALL {
        pollster::block_on(case.run(&Remembered::new(), 1_000));
    }
}

#[test]
fn every_case_has_its_own_name() {
    let mut names: Vec<&str> = NonceCase::ALL.iter().map(|case| case.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), NonceCase::ALL.len());
    assert!(NonceCase::ALL.iter().all(|case| NonceCase::named(case.name()) == Some(*case)));
}
