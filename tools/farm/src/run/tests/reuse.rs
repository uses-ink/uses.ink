//! The result cache seen through a farm: passes served again, dedupe, and what is never kept.

use super::*;

#[test]
fn a_pass_is_served_again_until_the_tree_changes_and_fresh_runs_it_anyway() {
    let rig = Rig::new("hit");
    std::fs::write(rig.root.join("w").join("a.rs"), "one").unwrap();
    let job = counting(&rig, "true");
    let first = again(&rig, &job, Flags::default());
    assert_eq!((first.served, runs(&rig)), (job::Served::Ran, 1), "a miss runs");
    let second = again(&rig, &job, Flags::default());
    assert!(matches!(second.served, job::Served::Cached { .. }), "{:?}", second.served);
    assert_eq!((runs(&rig), &second.id, &second.log), (1, &first.id, &first.log), "a hit runs nothing");
    assert!(second.to_string().contains(&format!("passed earlier on this tree ({}, ", first.id)), "{second}");
    assert_eq!(second.exit(), Exit::Passed);
    std::fs::write(rig.root.join("w").join("a.rs"), "two").unwrap();
    assert_eq!(again(&rig, &job, Flags::default()).served, job::Served::Ran, "a changed input is a miss");
    assert_eq!(runs(&rig), 2);
    let fresh = again(&rig, &job, Flags { fresh: true, ..Flags::default() });
    assert_eq!((fresh.served, runs(&rig)), (job::Served::Ran, 3), "--fresh bypasses the cache");
    assert!(matches!(again(&rig, &job, Flags::default()).served, job::Served::Cached { .. }), "and still records");
    assert_eq!(runs(&rig), 3);
}

#[test]
fn a_job_that_names_its_inputs_ignores_a_change_outside_them() {
    let rig = Rig::new("inputs");
    let tree = rig.root.join("w");
    std::fs::create_dir_all(tree.join("docs")).unwrap();
    std::fs::write(tree.join("docs").join("a.md"), "one").unwrap();
    std::fs::write(tree.join("a.rs"), "one").unwrap();
    let mut job = counting(&rig, "true");
    job.reuse =
        job::Reuse::Files(cache::inputs::glob::Selection::parse(&["**".to_owned(), "!docs/**".to_owned()]).unwrap());
    again(&rig, &job, Flags::default());
    std::fs::write(tree.join("docs").join("a.md"), "two").unwrap();
    assert!(matches!(again(&rig, &job, Flags::default()).served, job::Served::Cached { .. }));
    std::fs::write(tree.join("a.rs"), "two").unwrap();
    assert_eq!(again(&rig, &job, Flags::default()).served, job::Served::Ran);
    assert_eq!(runs(&rig), 2);
}

#[test]
fn failures_flaky_passes_and_jobs_that_opt_out_are_never_cached() {
    let rig = Rig::new("never");
    let failing = counting(&rig, "exit 1");
    assert_eq!(again(&rig, &failing, Flags::default()).exit(), Exit::Failed);
    assert_eq!(again(&rig, &failing, Flags::default()).served, job::Served::Ran);
    assert_eq!(runs(&rig), 2, "a failure runs again");
    let flaky = job::Job { label: "flaky".to_owned(), ..counting(&rig, "echo '        FLAKY 2/3 [ 0.5s] a b'") };
    assert_eq!(again(&rig, &flaky, Flags::default()).exit(), Exit::Passed);
    assert_eq!(again(&rig, &flaky, Flags::default()).served, job::Served::Ran);
    assert_eq!(runs(&rig), 4, "a pass that shows a retry runs again");
    let optout = job::Job { label: "optout".to_owned(), reuse: job::Reuse::Never, ..counting(&rig, "true") };
    again(&rig, &optout, Flags::default());
    assert_eq!(again(&rig, &optout, Flags::default()).served, job::Served::Ran);
    assert_eq!(runs(&rig), 6);
}

#[test]
fn an_identical_job_attaches_to_the_running_one_and_its_pass_is_kept() {
    let rig = Rig::new("dedupe");
    let latch = rig.root.join("latch");
    let job = counting(&rig, &format!("while [ ! -f {} ]; do sleep 0.01; done", latch.display()));
    let farm = rig.cached(2, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let first = farm.run(&job, &snap, job::Attach::Detach).unwrap();
    assert_eq!(first.verdict, Verdict::Running);
    let second = std::thread::scope(|scope| {
        let attached = scope.spawn(|| farm.run(&job, &snap, job::Attach::Hold(Duration::from_secs(60))));
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(&latch, "").unwrap();
        attached.join().unwrap().unwrap()
    });
    assert_eq!(second.served, job::Served::Attached);
    assert_eq!(second.id, first.id, "the same job, not a second copy");
    assert!(matches!(second.verdict, Verdict::Done { code: 0, .. }), "{:?}", second.verdict);
    assert_eq!(runs(&rig), 1, "the command ran once");
    assert!(
        matches!(again(&rig, &job, Flags::default()).served, job::Served::Cached { .. }),
        "the pass it waited for is kept"
    );
    assert_eq!(runs(&rig), 1);
}
