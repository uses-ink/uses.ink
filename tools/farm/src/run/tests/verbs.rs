//! The verbs over the fake transport and over the repo's own job file.

use super::*;
use crate::cli::error::Error;
use crate::place::Need;

#[test]
fn a_check_fans_out_one_job_per_job_with_golden_jobs_on_golden_hosts() {
    let x86 = Member::new(host("x86"), Fake::saying(vec![probe(12, "x86_64")]));
    let arm = Member::new(host("arm"), Fake::saying(vec![probe(12, "aarch64")]));
    let farm = farm(vec![x86, arm]);
    let jobs = [Need::Any, Need::Golden, Need::Any, Need::Any, Need::Golden].map(asking);
    let chosen = farm.place(&jobs.iter().collect::<Vec<_>>()).unwrap();
    let hosts: Vec<&str> = chosen.iter().map(|snap| snap.name.as_str()).collect();
    // lint takes the one free slot on `arm` or `x86`; both golden stages must be on x86
    assert_eq!(hosts.get(1), Some(&"x86"));
    assert_eq!(hosts.get(4), Some(&"x86"));
    assert!(hosts.contains(&"arm"), "{hosts:?}: free slots are used before anything queues");
}

#[test]
fn a_golden_job_with_only_an_arm_host_is_an_error_naming_the_rule() {
    let arm = Member::new(host("arm"), Fake::saying(vec![probe(12, "aarch64")]));
    let error = farm(vec![arm]).place(&[&asking(Need::Golden)]).unwrap_err();
    assert!(error.to_string().contains("no golden host"), "{error}");
}

#[test]
fn a_cargo_command_that_runs_tests_needs_a_golden_host() {
    let args = |line: &str| line.split(' ').map(str::to_owned).collect::<Vec<_>>();
    assert_eq!(Need::of_cargo(&args("nextest run -p cli")), Need::Golden);
    assert_eq!(Need::of_cargo(&args("test --doc")), Need::Golden);
    assert_eq!(Need::of_cargo(&args("+nightly test")), Need::Golden);
    assert_eq!(Need::of_cargo(&args("clippy -p engine")), Need::Any);
    assert_eq!(Need::of_cargo(&args("check")), Need::Any);
    assert_eq!(Need::of_cargo(&[]), Need::Any);
}

#[test]
fn the_value_of_a_flag_is_never_taken_for_the_cargo_verb() {
    let words = |list: &[&str]| list.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
    for flag in Need::VALUED {
        assert_eq!(Need::of_cargo(&words(&[flag, "x", "test"])), Need::Golden, "{flag} x test");
        assert_eq!(Need::of_cargo(&words(&[flag, "test", "check"])), Need::Any, "{flag} test check");
        assert_eq!(Need::of_cargo(&words(&["nextest", "run", flag, "x"])), Need::Golden, "{flag} after the verb");
    }
}

#[test]
fn a_job_runs_from_sync_to_summary_over_the_fake() {
    let events = Reply::ok(&[
        "@@farm:slot 0 3",
        "   Compiling x v0.1.0",
        "warning: unused variable: `a`",
        " --> src/lib.rs:1:1",
        "@@farm:done 0 12",
    ]);
    let member =
        Member::new(host("x86"), Fake::saying(vec![probe(12, "x86_64"), Reply::ok(&["@@farm:seed empty"]), events]));
    let farm = farm(vec![member]);
    let batch = farm.cargo(&["clippy".to_owned(), "-p".to_owned(), "engine".to_owned()]).unwrap();
    let done = batch.done.first().unwrap();
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    assert_eq!((done.slot, done.queued), (Some(0), Duration::from_secs(3)));
    assert_eq!(done.verdict, Verdict::Done { code: 0, ran: Duration::from_secs(12) });
    assert_eq!(done.summary.diagnostics.warnings, 1);
    assert_eq!(done.command, "cargo clippy -p engine");
    assert_eq!(farm.report(&batch), Exit::Passed);
    std::fs::remove_dir_all(&farm.tree.root).ok();
}

#[test]
fn a_connection_that_ends_without_a_result_is_lost_and_a_failed_command_is_failed() {
    let lost = Member::new(
        host("a"),
        Fake::saying(vec![
            probe(12, "x86_64"),
            Reply::ok(&["@@farm:seed empty"]),
            Reply::ok(&["@@farm:slot 0 0", "output"]),
        ]),
    );
    let batch = farm(vec![lost]).cargo(&["check".to_owned()]).unwrap();
    assert!(matches!(batch.failed.first(), Some(Error::Lost { .. })), "{:?}", batch.failed);
    let failed = Member::new(
        host("a"),
        Fake::saying(vec![
            probe(12, "x86_64"),
            Reply::ok(&["@@farm:seed empty"]),
            Reply::ok(&["@@farm:slot 0 0", "@@farm:done 101 2"]),
        ]),
    );
    let farm = farm(vec![failed]);
    let batch = farm.cargo(&["check".to_owned()]).unwrap();
    assert_eq!(farm.report(&batch), Exit::Failed);
    std::fs::remove_dir_all(&farm.tree.root).ok();
}

#[test]
fn a_body_that_never_started_is_an_error_naming_the_job_not_a_lost_connection() {
    let silent = Member::new(
        host("a"),
        Fake::saying(vec![probe(12, "x86_64"), Reply::ok(&["@@farm:seed empty"]), Reply::ok(&["@@farm:nobody"])]),
    );
    let farm = farm(vec![silent]);
    let batch = farm.cargo(&["check".to_owned()]).unwrap();
    let Some(error @ Error::NoBody { job }) = batch.failed.first() else { panic!("{:?}", batch.failed) };
    assert!(error.to_string().contains(&job.to_string()));
    assert_eq!(error.exit(), Exit::Farm);
    std::fs::remove_dir_all(&farm.tree.root).ok();
}

#[test]
fn a_check_runs_the_jobs_of_the_repos_file_and_exits_with_the_worst() {
    let rig = Rig::new("check");
    let config = rig.root.join("w").join(".config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("farm.toml"),
        "[job.good]\nrun = \"true\"\ngolden = false\n\n[job.bad]\nrun = \"false\"\ngolden = false\n",
    )
    .unwrap();
    // `bad` would cancel `good` if it were still queued; this test is about the exits
    let farm = rig.farm(2, Flags { keep_going: true, ..Flags::default() });
    let batch = farm.check(&[]).unwrap();
    let labels: Vec<&str> = batch.done.iter().map(|done| done.label.as_str()).collect();
    assert_eq!(labels, ["good", "bad"]);
    assert_eq!(batch.done.iter().map(|done| done.verdict.exit()).collect::<Vec<_>>(), [Exit::Passed, Exit::Failed]);
    let none = farm.check(&["nope".parse().unwrap()]).unwrap_err();
    assert_eq!(none.exit(), Exit::Usage);
}

#[test]
fn bless_pulls_only_listed_snapshots_and_refuses_a_hostile_list() {
    let ok = Member::new(
        host("x86"),
        Fake::saying(vec![
            probe(12, "x86_64"),
            Reply::ok(&["@@farm:seed empty"]),
            Reply::ok(&["@@farm:slot 0 0", "@@farm:done 0 5"]),
            Reply::ok(&["./a/x.snap"]),
        ]),
    );
    let farm_ok = farm(vec![ok]);
    let batch = farm_ok.bless(&[]).unwrap();
    assert_eq!(batch.blessed, ["a/x.snap"]);
    assert_eq!(*farm_ok.fleet.members().first().unwrap().transport().pulled.lock().unwrap(), ["a/x.snap\n"]);
    std::fs::remove_dir_all(&farm_ok.tree.root).ok();
    let bad = Member::new(
        host("x86"),
        Fake::saying(vec![
            probe(12, "x86_64"),
            Reply::ok(&["@@farm:seed empty"]),
            Reply::ok(&["@@farm:slot 0 0", "@@farm:done 0 5"]),
            Reply::ok(&["./src/lib.rs"]),
        ]),
    );
    let farm_bad = farm(vec![bad]);
    let batch = farm_bad.bless(&[]).unwrap();
    assert!(matches!(batch.failed.first(), Some(Error::Changes { .. })));
    assert!(farm_bad.fleet.members().first().unwrap().transport().pulled.lock().unwrap().is_empty());
    std::fs::remove_dir_all(&farm_bad.tree.root).ok();
}

/// A host that formats: it runs the job, then lists `listed`.
fn formats(listed: &[&str]) -> Member<Fake> {
    Member::new(
        host("x86"),
        Fake::saying(vec![
            probe(12, "x86_64"),
            Reply::ok(&["@@farm:seed empty"]),
            Reply::ok(&["@@farm:slot 0 0", "@@farm:done 0 1"]),
            Reply::ok(listed),
        ]),
    )
}

fn stamped(farm: &Farm<Fake, LeastLoaded>, name: &str, at: SystemTime) {
    std::fs::create_dir_all(&farm.tree.root).unwrap();
    let path = farm.tree.root.join(name);
    std::fs::write(&path, "x").unwrap();
    std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
}

#[test]
fn bless_refuses_to_overwrite_a_snapshot_edited_after_the_sync_started() {
    let host = Member::new(
        host("x86"),
        Fake::saying(vec![
            probe(12, "x86_64"),
            Reply::ok(&["@@farm:seed empty"]),
            Reply::ok(&["@@farm:slot 0 0", "@@farm:done 0 5"]),
            Reply::ok(&["./a.snap", "./new.snap"]),
        ]),
    );
    let farm = farm(vec![host]);
    stamped(&farm, "a.snap", SystemTime::now() + Duration::from_secs(3600));
    let batch = farm.bless(&[]).unwrap();
    let Some(Error::Edited { paths, .. }) = batch.failed.first() else { panic!("{:?}", batch.failed) };
    assert_eq!(paths, &["a.snap"], "a snapshot that is not here yet is not an edit");
    assert_eq!(batch.blessed, Vec::<String>::new());
    assert!(farm.fleet.members().first().unwrap().transport().pulled.lock().unwrap().is_empty());
    std::fs::remove_dir_all(&farm.tree.root).ok();
}

#[test]
fn fmt_brings_back_only_the_listed_rust_files() {
    let ok = farm(vec![formats(&["./a.rs"])]);
    stamped(&ok, "a.rs", SystemTime::now() - Duration::from_secs(60));
    let batch = ok.fmt().unwrap();
    assert_eq!(batch.formatted, ["a.rs"]);
    assert!(batch.failed.is_empty(), "{:?}", batch.failed);
    let call = ok.fleet.members().first().unwrap().transport().calls.lock().unwrap().get(3).cloned().unwrap();
    assert_eq!(call.args.last().map(String::as_str), Some("*.rs"));
    assert_eq!(*ok.fleet.members().first().unwrap().transport().pulled.lock().unwrap(), ["a.rs\n"]);
    assert_eq!(ok.report(&batch), Exit::Passed);
    std::fs::remove_dir_all(&ok.tree.root).ok();
    let hostile = farm(vec![formats(&["./Cargo.toml"])]);
    let batch = hostile.fmt().unwrap();
    assert!(matches!(batch.failed.first(), Some(Error::Changes { .. })));
    assert!(hostile.fleet.members().first().unwrap().transport().pulled.lock().unwrap().is_empty());
    std::fs::remove_dir_all(&hostile.tree.root).ok();
}

#[test]
fn fmt_refuses_to_overwrite_a_file_edited_after_the_sync_started() {
    let farm = farm(vec![formats(&["./a.rs", "./b.rs"])]);
    stamped(&farm, "a.rs", SystemTime::now() - Duration::from_secs(60));
    stamped(&farm, "b.rs", SystemTime::now() + Duration::from_secs(3600));
    let batch = farm.fmt().unwrap();
    assert_eq!(batch.formatted, Vec::<String>::new());
    let Some(Error::Edited { paths, .. }) = batch.failed.first() else { panic!("{:?}", batch.failed) };
    assert_eq!(paths, &["b.rs"]);
    assert!(farm.fleet.members().first().unwrap().transport().pulled.lock().unwrap().is_empty());
    assert_eq!(farm.report(&batch), Exit::Refused);
    std::fs::remove_dir_all(&farm.tree.root).ok();
}
