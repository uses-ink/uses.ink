//! The suite over the recorded advertisements.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_watch_worker as watch_worker;

use watch_worker::Case;

#[test]
fn every_case_passes() {
    for case in Case::ALL {
        pollster::block_on(case.run());
    }
}

#[test]
fn every_case_has_its_own_name() {
    let mut names: Vec<&str> = Case::ALL.iter().map(|case| case.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Case::ALL.len());
    assert!(Case::ALL.iter().all(|case| Case::named(case.name()) == Some(*case)));
}
