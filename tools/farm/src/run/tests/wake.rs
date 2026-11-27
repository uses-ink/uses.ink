//! Sleeping hosts: woken by their command, then probed and used.

use super::*;
use crate::cli::Seconds;
use crate::cli::error::Chain;
use crate::cli::error::Error;
use crate::host::Snapshot;
use crate::place::Need;

#[test]
fn a_pinned_sleeping_host_is_woken_by_the_command_then_probed_and_used() {
    let wakeup = Wakeup::new(0);
    let asleep = Member::new(wakeup.host("farm-a"), Fake::saying(vec![silent(), probe(16, "x86_64")]));
    let flags = Flags { host: Some("farm-a".parse().unwrap()), ..Flags::default() };
    let chosen = Farm::with(vec![asleep], flags, None).place(&[&asking(Need::Golden)]).unwrap();
    assert_eq!(wakeup.woken(), ["farm-a"], "the command got the host name as its only argument");
    let snap = chosen.first().unwrap();
    assert_eq!((snap.name.as_str(), snap.asleep, snap.cores), ("farm-a", None, 16));
}

#[test]
fn a_failed_wake_leaves_the_host_out_and_the_job_is_placed_without_it_not_hung() {
    let wakeup = Wakeup::new(3);
    let dir = wakeup.dir.join("cache");
    let asleep = Member::new(wakeup.host("farm-a"), Fake::saying(vec![silent()]));
    let arm = Member::new(host("arm"), Fake::saying(vec![probe(8, "aarch64")]));
    let sleeper = wakeup.host("farm-a");
    let seen = Snapshot::asleep(&sleeper, crate::host::Arch::X86, (16, 1), Seconds(120));
    let farm = Farm::with(vec![asleep, arm], Flags::default(), Some(&dir));
    let mut up = seen.clone();
    up.asleep = None;
    farm.cache.as_ref().unwrap().seen.note(&up).unwrap();
    // a job any host may run goes to the one that is up, and nothing is woken
    let any = farm.place(&[&asking(Need::Any)]).unwrap();
    assert_eq!(any.first().map(|snap| snap.name.as_str()), Some("arm"));
    assert_eq!(wakeup.woken(), Vec::<String>::new());
    // a golden job can only use the sleeper: it is woken, fails, and then no host fits
    let farm = Farm::with(
        vec![
            Member::new(wakeup.host("farm-a"), Fake::saying(vec![silent()])),
            Member::new(host("arm"), Fake::saying(vec![probe(8, "aarch64")])),
        ],
        Flags::default(),
        Some(&dir),
    );
    let error = farm.place(&[&asking(Need::Golden)]).unwrap_err();
    assert!(matches!(error, Error::NoHost { need: Need::Golden, .. }), "{error}");
    assert_eq!(wakeup.woken().len(), 1);
}

#[test]
fn a_pinned_host_that_cannot_be_woken_is_the_typed_error_naming_command_and_status() {
    let wakeup = Wakeup::new(3);
    let asleep = Member::new(wakeup.host("farm-a"), Fake::saying(vec![silent()]));
    let flags = Flags { host: Some("farm-a".parse().unwrap()), ..Flags::default() };
    let error = Farm::with(vec![asleep], flags, None).place(&[&asking(Need::Any)]).unwrap_err();
    let shown = Chain(&error).to_string();
    assert!(matches!(error, Error::Wake { .. }), "{shown}");
    assert!(shown.contains("`farm-a` could not be woken") && shown.contains("wake.sh farm-a"), "{shown}");
    assert!(shown.contains("exit status: 3"), "{shown}");
}
