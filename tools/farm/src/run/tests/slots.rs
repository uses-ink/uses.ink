//! Slots, the queue, detach and the limit, on the real shell.

use super::*;

#[test]
fn a_child_that_outlives_its_job_neither_holds_the_slot_nor_stays_alive() {
    let rig = Rig::new("orphan");
    let pidfile = rig.root.join("child");
    let script = format!("sleep 300 & echo $! > {}; exit 0", pidfile.display());
    let finished = run_one(&rig, &job("orphan", &script), job::Attach::Hold(Duration::from_secs(30)), 1);
    assert_eq!(finished.verdict.exit(), Exit::Passed);
    let probe = rig.member().probe().unwrap();
    assert_eq!(probe.busy, 0, "the lease is free as soon as the job ends");
    let pid = std::fs::read_to_string(&pidfile).unwrap().trim().to_owned();
    let state = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let gone = state.is_empty() || state.rsplit_once(')').is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'));
    assert!(gone, "the orphan was killed with its job: {state}");
}

#[test]
fn a_job_runs_in_a_slot_with_its_environment_and_streams_its_output() {
    let rig = Rig::new("env");
    let finished = run_one(
        &rig,
        &job("env", "echo hi; echo \"$CARGO_BUILD_JOBS $CARGO_TERM_COLOR $INSTA_UPDATE\"; exit 3"),
        job::Attach::Hold(Duration::from_secs(30)),
        2,
    );
    assert_eq!(finished.verdict.exit(), Exit::Failed);
    assert!(matches!(finished.verdict, Verdict::Done { code: 3, .. }));
    assert_eq!(finished.slot, Some(0));
    let log = std::fs::read_to_string(&finished.log).unwrap();
    assert!(log.starts_with("hi\n"), "{log}");
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    assert!(log.contains(&format!("{} never no", (cores / 2).max(1))), "{log}");
}

#[test]
fn the_limit_stops_a_job_and_says_so() {
    let rig = Rig::new("limit");
    let flags = Flags { limit: Some(cli::Seconds(1)), ..Flags::default() };
    let farm = rig.farm(1, flags);
    let snap = farm.fleet_probe().remove(0);
    let finished = farm.run(&job("slow", "sleep 30"), &snap, job::Attach::Hold(Duration::from_secs(30))).unwrap();
    assert_eq!(finished.verdict.exit(), Exit::TimedOut);
}

#[test]
fn a_job_that_never_stops_printing_is_still_let_go_at_the_timeout() {
    let rig = Rig::new("chatty");
    let farm = rig.farm(1, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let latch = rig.root.join("latch");
    let script = format!("while [ ! -f {} ]; do echo x; done", latch.display());
    let started = std::time::Instant::now();
    let finished = farm.run(&job("chatty", &script), &snap, job::Attach::Hold(Duration::from_millis(50))).unwrap();
    std::fs::write(&latch, "").unwrap();
    assert_eq!(finished.verdict, Verdict::Running);
    assert!(started.elapsed() < Duration::from_millis(900), "{:?}", started.elapsed());
    rig.member().wait(&finished.id, cli::Seconds(20)).unwrap();
}

#[test]
fn a_detached_job_outlives_the_client_and_wait_returns_its_result() {
    let rig = Rig::new("detach");
    let latch = rig.root.join("latch");
    let script = format!("while [ ! -f {} ]; do sleep 0.01; done; echo finished; exit 7", latch.display());
    let started = run_one(&rig, &job("bg", &script), job::Attach::Detach, 1);
    assert_eq!(started.verdict, Verdict::Running);
    let member = rig.member();
    let said = member.wait(&started.id, cli::Seconds(0)).unwrap();
    assert_eq!(said.events, [job::event::Event::Running], "not done yet: still running, safe to call again");
    std::fs::write(&latch, "").unwrap();
    let said = member.wait(&started.id, cli::Seconds(20)).unwrap();
    assert!(matches!(said.events.first(), Some(job::event::Event::Done { code: 7, .. })), "{:?}", said.events);
    assert_eq!(member.log(&started.id, None).unwrap().output, ["finished"]);
    assert_eq!(member.log(&started.id, Some(1)).unwrap().output, ["finished"]);
    let missing = JobIdHack::other(&started.id);
    assert_eq!(member.wait(&missing, cli::Seconds(1)).unwrap().events, [job::event::Event::NoJob]);
}

/// A job id of the same host that no job has.
struct JobIdHack;

impl JobIdHack {
    fn other(id: &cli::name::JobId) -> cli::name::JobId {
        format!("{}.deadbeef", id.host).parse().unwrap()
    }
}

#[test]
fn one_slot_runs_jobs_one_at_a_time_and_the_queue_is_first_come_first_served() {
    let rig = Rig::new("queue");
    let member = rig.member();
    let order = rig.root.join("order");
    let farm = rig.farm(1, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let latch = rig.root.join("latch");
    let hold = format!("while [ ! -f {} ]; do sleep 0.01; done; ", latch.display());
    let script = |n: u32, gate: &str| format!("echo start{n} >> {o}; {gate}echo end{n} >> {o}", o = order.display());
    let mut ids = Vec::new();
    for n in 0..3 {
        let gate = if n == 0 { hold.as_str() } else { "" };
        let finished = farm.run(&job("q", &script(n, gate)), &snap, job::Attach::Detach).unwrap();
        ids.push(finished.id);
        assert!(within(|| member.probe().is_ok_and(|probe| probe.busy + probe.queued == n + 1)), "job {n} is placed");
    }
    std::fs::write(&latch, "").unwrap();
    for id in &ids {
        member.wait(id, cli::Seconds(30)).unwrap();
    }
    let text = std::fs::read_to_string(&order).unwrap();
    assert_eq!(text, "start0\nend0\nstart1\nend1\nstart2\nend2\n", "never two at once, oldest first");
}

#[test]
fn a_dropped_client_stops_its_job_and_frees_the_slot() {
    let rig = Rig::new("drop");
    let member = rig.member();
    let farm = rig.farm(1, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    // a held run that outlives its timeout is let go: the job survives, as `detach` says
    let latch = rig.root.join("latch");
    let script = format!("while [ ! -f {} ]; do sleep 0.01; done", latch.display());
    let finished = farm.run(&job("held", &script), &snap, job::Attach::Hold(Duration::from_millis(50))).unwrap();
    assert_eq!(finished.verdict, Verdict::Running);
    std::fs::write(&latch, "").unwrap();
    let said = member.wait(&finished.id, cli::Seconds(20)).unwrap();
    assert!(matches!(said.events.first(), Some(job::event::Event::Done { code: 0, .. })), "{:?}", said.events);
    // the slot is free again
    let probe = member.probe().unwrap();
    assert_eq!((probe.busy, probe.queued), (0, 0));
}

#[test]
fn a_killed_client_stops_the_job_and_frees_the_slot() {
    let rig = Rig::new("kill");
    let member = rig.member();
    let farm = rig.farm(1, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let id = cli::name::JobId::new(HostName::local(), 1, 1);
    let job = job("held", "sleep 60");
    let start = host::Start { tree: &farm_tree(&rig), snap: &snap, job: &job, id: &id };
    let call = member.launch(&start, job::Attach::Hold(Duration::from_secs(60)), cli::Seconds(120));
    let argv = call.argv();
    let mut child = std::process::Command::new("sh")
        .args(argv.iter().skip(1))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    assert!(within(|| member.probe().is_ok_and(|probe| probe.busy == 1)), "the job took the slot");
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(within(|| member.probe().is_ok_and(|probe| probe.busy == 0)), "the slot came free");
    let log = member.wait(&id, cli::Seconds(5)).unwrap();
    assert!(matches!(log.events.first(), Some(job::event::Event::Done { code: 143, .. })), "{:?}", log.events);
}

#[test]
fn a_ticket_of_a_dead_job_does_not_hold_up_the_queue() {
    let rig = Rig::new("stale");
    let member = rig.member();
    member.probe().unwrap();
    let queue = rig.root.join("farm").join("queue");
    std::fs::write(queue.join("0000000000000000001-ghost"), "999999999\n").unwrap();
    let farm = rig.farm(1, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    let finished = farm.run(&job("late", "true"), &snap, job::Attach::Hold(Duration::from_secs(30))).unwrap();
    assert_eq!(finished.verdict.exit(), Exit::Passed);
    assert!(!queue.join("0000000000000000001-ghost").exists(), "the stale ticket was removed");
}

#[test]
fn the_slot_locks_are_where_the_host_dir_says() {
    let rig = Rig::new("slots");
    let dir = rig.dir();
    let path = |n| std::path::PathBuf::from(dir.slot(n));
    assert!(!path(0).exists());
    let farm = rig.farm(2, Flags::default());
    let snap = farm.fleet_probe().remove(0);
    farm.run(&job("x", "true"), &snap, job::Attach::Hold(Duration::from_secs(30))).unwrap();
    assert!(path(0).exists() && path(1).exists(), "the probe tries every slot's lock, which makes the file");
    assert_eq!(dir.slot(1), format!("{}/slots/1.lock", rig.root.join("farm").display()));
}
