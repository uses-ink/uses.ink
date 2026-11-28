//! A failing stage cancels the stages beside it, on the real shell.

use super::*;
use crate::run::verb::Stop;

/// A job that notes its start, runs `sleep 60` until `flag` exists, and counts its runs.
fn lingering(rig: &Rig, started: &std::path::Path, flag: &std::path::Path) -> job::Job {
    let script = format!("[ -f {} ] && exit 0; touch {}; sleep 60", flag.display(), started.display());
    counting(rig, &script)
}

#[test]
fn a_failing_stage_cancels_the_one_still_running_and_that_one_is_never_failed_or_cached() {
    let rig = Rig::new("cancel");
    let (started, flag) = (rig.root.join("started"), rig.root.join("flag"));
    let farm = rig.cached(2, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let slow = lingering(&rig, &started, &flag);
    let waits = format!("while [ ! -f {} ]; do sleep 0.01; done; exit 1", started.display());
    let timer = std::time::Instant::now();
    let batch = farm.spread(&[slow.clone(), job("fail", &waits)], &[snap.clone(), snap], Stop::Siblings);
    assert!(timer.elapsed() < Duration::from_secs(4), "{:?}", timer.elapsed());
    assert_eq!(batch.done.first().map(|done| done.verdict), Some(Verdict::Cancelled));
    assert!(matches!(batch.done.get(1).map(|done| done.verdict), Some(Verdict::Done { code: 1, .. })));
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    assert_eq!(farm.report(&batch), Exit::Failed, "the failed stage sets the exit, the cancelled one adds none");
    assert!(within(|| rig.member().probe().is_ok_and(|probe| probe.busy == 0)), "its slot on the host is free");
    // not cached: with the reason to wait gone, the same job runs again instead of replaying a pass
    std::fs::write(&flag, "").unwrap();
    let rerun = again(&rig, &slow, Flags::default());
    assert_eq!(rerun.exit(), Exit::Passed);
    assert!(matches!(rerun.served, job::Served::Ran));
    assert_eq!(runs(&rig), 2);
}

#[test]
fn without_stop_a_failing_stage_leaves_its_siblings_to_finish() {
    let rig = Rig::new("keep");
    let farm = rig.farm(2, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let jobs = [job("slow", "sleep 0.3; exit 0"), job("fail", "exit 1")];
    let batch = farm.spread(&jobs, &[snap.clone(), snap], Stop::Never);
    assert!(matches!(batch.done.first().map(|done| done.verdict), Some(Verdict::Done { code: 0, .. })));
    assert!(matches!(batch.done.get(1).map(|done| done.verdict), Some(Verdict::Done { code: 1, .. })));
}

#[test]
fn a_stage_that_has_not_started_when_the_check_is_cancelled_never_starts() {
    let rig = Rig::new("early");
    let farm = rig.farm(1, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    farm.cancel.set();
    let marker = rig.root.join("ran");
    let finished = farm
        .run(&job("late", &format!("touch {}", marker.display())), &snap, job::Attach::Hold(Duration::from_secs(5)))
        .unwrap();
    assert_eq!(finished.verdict, Verdict::Cancelled);
    assert!(!marker.exists());
}

#[test]
fn the_cancelled_stages_are_named_once() {
    let said = crate::run::verb::Stopped {
        failed: vec!["lint".to_owned()],
        stopped: vec!["test".to_owned(), "sites".to_owned()],
    };
    assert_eq!(
        said.to_string(),
        "check: `lint` failed, so `test` and `sites` cancelled (`--keep-going` lets every stage finish)"
    );
}
