//! `farm <verb>`: the build farm client. A repo's jobs and any cargo command run on the hosts of
//! the user's config, each in a slot of its own, so six agents do not make one machine thrash and
//! a test budget is measured on a quiet core. It knows no project: the jobs are the repo's own
//! `.config/farm.toml`. See docs/workflow.md for the verbs and docs/decisions.md (0047) for why
//! it is built this way.
//!
//! - [`transport`]: how a call reaches a host: [`transport::Local`] and [`transport::Ssh`]
//!   behind the [`transport::Transport`] trait.
//! - [`program`]: the shell programs that run on a host (slots, queue, jobs), as fixed text with
//!   positional parameters.
//! - [`host`]: [`host::Member`] and [`host::Fleet`], generic over the transport.
//! - [`place`]: [`place::Placement`], which host gets a job; [`history`], how long jobs took there.
//! - [`run`]: [`run::Farm`], the verbs. [`summary`], [`job`] and [`event`] read what comes back.
//! - [`jobs`]: the repo's job file.
//! - [`version`]: the hash of the source the binary was built from.
//! - [`workspace`]: `farm workspace`, which needs no host.

mod cache;
mod cli;
mod config;
mod host;
mod job;
mod place;
mod run;

use std::process::ExitCode;
use std::sync::Arc;

use cache::Cache;
use cache::inputs::{Inputs, Jj};
use cli::console::Console;
use cli::error::{Chain, Error};
use cli::exit::Exit;
use cli::{Command, Verb};
use config::hosts::{Config, Environment};
use config::jobs::Jobs;
use config::tree::Tree;
use config::workspace::Workspaces;
use host::transport::{Exec, Link, Local, Ssh};
use host::{Fleet, Member};
use host::{Host, Via};
use place::Fastest;
use run::{Farm, Setup};

/// The entry point.
#[derive(Debug, Clone, Copy)]
struct Entry;

impl Entry {
    /// Runs the farm command `args`, from the workspace the current directory is in; the exit
    /// code is the farm's own (see [`Exit`]).
    fn main(args: Vec<String>) -> ExitCode {
        let console = Arc::new(Console::default());
        let exit = match Self::exec(args) {
            Ok(exit) => exit,
            Err(error) => {
                console.fail(Chain(&error));
                error.exit()
            }
        };
        exit.into()
    }

    fn exec(args: Vec<String>) -> Result<Exit, Error> {
        let Command { flags, verb } = Command::parse(args)?;
        let console = Arc::new(Console::new(flags.verbose));
        if verb == Verb::Version {
            console.out(cli::version::Version::current());
            return Ok(Exit::Passed);
        }
        let here = std::env::current_dir().map_err(|source| Error::Io { path: ".".into(), source })?;
        let tree = Tree::find(&here)?;
        match &verb {
            Verb::Make(name) => {
                let made = Workspaces::new(&tree).make(name, flags.at.as_deref())?;
                console.out(made);
                return Ok(Exit::Passed);
            }
            Verb::Forget(name) => {
                let jobs = Jobs::load(&tree.root)?;
                let path = Workspaces::new(&tree).forget(name, jobs.disposable())?;
                console.out(format!("workspace `{name}` forgotten and {} removed", path.display()));
                return Ok(Exit::Passed);
            }
            _ => {}
        }
        let env = Environment::current();
        let config = Config::load(&env)?;
        let settings = Jobs::load_optional(&tree.root)?.map_or_default(|jobs| jobs.settings().clone());
        let cache = Self::cache(&env, &tree, &settings)?;
        let placement = Fastest::new(Self::table(cache.as_ref(), &console));
        let exec = Exec::new(Arc::clone(&console));
        let members = config.hosts.iter().map(|host| Self::member(host, &exec)).collect();
        let farm = Farm::new(Setup { fleet: Fleet::new(members), placement, console, flags, tree, settings, cache });
        farm.go(&verb)
    }

    /// The timings placement uses; a history that cannot be read is said and placement goes on
    /// without it.
    fn table(cache: Option<&Cache>, console: &Console) -> place::history::Table {
        let Some(cache) = cache else {
            return place::history::Table::default();
        };
        cache.history.load(&place::history::Wall).unwrap_or_else(|fault| {
            console.note(format!("timing history not used: {}", cache::Causes(&cache::Trouble::Store(fault))));
            place::history::Table::default()
        })
    }

    /// What the farm remembers on this machine, unless there is nowhere to keep it.
    fn cache(env: &Environment, tree: &Tree, settings: &config::jobs::Settings) -> Result<Option<Cache>, Error> {
        let Some(dir) = env.cache_dir(tree.repo.as_str()) else {
            return Ok(None);
        };
        let inputs =
            Inputs::new(Arc::new(Jj), &settings.exclude).map_err(|fault| Error::Cache(Box::new(fault.into())))?;
        Ok(Some(Cache::at(&dir, settings.limits, settings.memory, inputs)))
    }

    fn member(host: &Host, exec: &Exec) -> Member<Link> {
        let link = match &host.via {
            Via::Local => Link::Local(Local::new(exec.clone())),
            Via::Ssh(destination) => Link::Ssh(Ssh::new(exec.clone(), destination.clone())),
        };
        Member::new(host.clone(), link)
    }
}

fn main() -> ExitCode {
    Entry::main(std::env::args().skip(1).collect())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::cli::exit::Exit;
    use crate::cli::name::HostName;
    use crate::cli::{Action, Flags, Verb};
    use crate::job::Verdict;
    use crate::place::LeastLoaded;

    /// A farm of this machine alone, in a temp dir: the real `sh`, `flock`, `timeout` and `tail`.
    struct Rig {
        root: std::path::PathBuf,
    }

    impl Rig {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("farm-rig-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&root).ok();
            std::fs::create_dir_all(root.join("w")).unwrap();
            Self { root }
        }

        fn farm(&self, slots: u32, flags: Flags) -> Farm<Link, LeastLoaded> {
            self.build(slots, flags, false)
        }

        /// A farm that keeps passes under the rig, reading the tree with a plain walk.
        fn cached(&self, slots: u32, flags: Flags) -> Farm<Link, LeastLoaded> {
            self.build(slots, flags, true)
        }

        fn build(&self, slots: u32, flags: Flags, cached: bool) -> Farm<Link, LeastLoaded> {
            let settings = config::jobs::Settings::default();
            let cache = cached.then(|| {
                let inputs = cache::inputs::Inputs::new(Arc::new(cache::inputs::Walk), &settings.exclude).unwrap();
                Cache::at(&self.root.join("cache"), settings.limits, settings.memory, inputs)
            });
            let host = Host { slots: Some(slots), dir: self.dir(), ..Host::local() };
            let console = Arc::new(Console::default());
            let local = Link::Local(Local::new(Exec::new(Arc::clone(&console))));
            let tree = Tree { root: self.root.join("w"), repo: "r".parse().unwrap(), workspace: "w".parse().unwrap() };
            Farm::new(Setup {
                fleet: Fleet::new(vec![Member::new(host, local)]),
                placement: LeastLoaded,
                console,
                flags,
                tree,
                settings,
                cache,
            })
        }

        fn dir(&self) -> host::HostDir {
            self.root.join("farm").to_str().unwrap().parse().unwrap()
        }

        fn member(&self) -> Member<Link> {
            let console = Arc::new(Console::default());
            Member::new(Host { dir: self.dir(), ..Host::local() }, Link::Local(Local::new(Exec::new(console))))
        }
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    fn cargo_free(words: &[&str]) -> Vec<String> {
        // `sh -c` stands in for cargo so the programs run without a toolchain
        let mut args = vec!["sh".to_owned(), "-c".to_owned()];
        args.extend(words.iter().map(|word| (*word).to_owned()));
        args
    }

    fn job(label: &str, script: &str) -> job::Job {
        job::Job {
            label: label.to_owned(),
            words: cargo_free(&[script]),
            need: place::Need::Any,
            env: vec![],
            limit: None,
            reuse: job::Reuse::Never,
            key: place::history::Key::stage(label),
        }
    }

    /// A job that counts its runs in the rig's `count` file, then runs `more`.
    fn counting(rig: &Rig, more: &str) -> job::Job {
        let mut counted = job("count", &format!("echo x >> {}; {more}", rig.root.join("count").display()));
        counted.reuse = job::Reuse::Whole;
        counted
    }

    fn runs(rig: &Rig) -> usize {
        std::fs::read_to_string(rig.root.join("count")).map_or(0, |text| text.lines().count())
    }

    /// One job through a new farm of the rig, as a new process would run it.
    fn again(rig: &Rig, job: &job::Job, flags: Flags) -> job::Finished {
        let farm = rig.cached(1, flags);
        let probed = farm.fleet_probe();
        farm.run_for_test(job, probed.first().unwrap(), job::Attach::Hold(Duration::from_secs(60))).unwrap()
    }

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
        job.reuse = job::Reuse::Files(
            cache::inputs::glob::Selection::parse(&["**".to_owned(), "!docs/**".to_owned()]).unwrap(),
        );
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
        let first = farm.run_for_test(&job, &snap, job::Attach::Detach).unwrap();
        assert_eq!(first.verdict, Verdict::Running);
        let second = std::thread::scope(|scope| {
            let attached = scope.spawn(|| farm.run_for_test(&job, &snap, job::Attach::Hold(Duration::from_secs(60))));
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
        let gone =
            state.is_empty() || state.rsplit_once(')').is_some_and(|(_, rest)| rest.trim_start().starts_with('Z'));
        assert!(gone, "the orphan was killed with its job: {state}");
    }

    fn run_one(rig: &Rig, job: &job::Job, attach: job::Attach, slots: u32) -> job::Finished {
        let farm = rig.farm(slots, Flags::default());
        let probed = farm.fleet_probe();
        farm.run_for_test(job, probed.first().unwrap(), attach).unwrap()
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
        let finished =
            farm.run_for_test(&job("slow", "sleep 30"), &snap, job::Attach::Hold(Duration::from_secs(30))).unwrap();
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
        let finished =
            farm.run_for_test(&job("chatty", &script), &snap, job::Attach::Hold(Duration::from_millis(50))).unwrap();
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
        let script =
            |n: u32, gate: &str| format!("echo start{n} >> {o}; {gate}echo end{n} >> {o}", o = order.display());
        let mut ids = Vec::new();
        for n in 0..3 {
            let gate = if n == 0 { hold.as_str() } else { "" };
            let finished = farm.run_for_test(&job("q", &script(n, gate)), &snap, job::Attach::Detach).unwrap();
            ids.push(finished.id);
            assert!(
                within(|| member.probe().is_ok_and(|probe| probe.busy + probe.queued == n + 1)),
                "job {n} is placed"
            );
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
        let finished =
            farm.run_for_test(&job("held", &script), &snap, job::Attach::Hold(Duration::from_millis(50))).unwrap();
        assert_eq!(finished.verdict, Verdict::Running);
        std::fs::write(&latch, "").unwrap();
        let said = member.wait(&finished.id, cli::Seconds(20)).unwrap();
        assert!(matches!(said.events.first(), Some(job::event::Event::Done { code: 0, .. })), "{:?}", said.events);
        // the slot is free again
        let probe = member.probe().unwrap();
        assert_eq!((probe.busy, probe.queued), (0, 0));
    }

    /// Polls `check` for up to five seconds.
    fn within(mut check: impl FnMut() -> bool) -> bool {
        for _ in 0..1000 {
            if check() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
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
        let finished =
            farm.run_for_test(&job("late", "true"), &snap, job::Attach::Hold(Duration::from_secs(30))).unwrap();
        assert_eq!(finished.verdict.exit(), Exit::Passed);
        assert!(!queue.join("0000000000000000001-ghost").exists(), "the stale ticket was removed");
    }

    fn farm_tree(rig: &Rig) -> Tree {
        Tree { root: rig.root.join("w"), repo: "r".parse().unwrap(), workspace: "w".parse().unwrap() }
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
        let farm = rig.farm(2, Flags::default());
        let batch = farm.go_for_test(&Verb::Check(vec![])).unwrap();
        let labels: Vec<&str> = batch.done.iter().map(|done| done.label.as_str()).collect();
        assert_eq!(labels, ["good", "bad"]);
        assert_eq!(batch.done.iter().map(|done| done.verdict.exit()).collect::<Vec<_>>(), [Exit::Passed, Exit::Failed]);
        let none = farm.go_for_test(&Verb::Check(vec!["nope".parse().unwrap()])).unwrap_err();
        assert_eq!(none.exit(), Exit::Usage);
    }

    #[test]
    fn the_slot_locks_are_where_the_host_dir_says() {
        let rig = Rig::new("slots");
        let dir = rig.dir();
        let path = |n| std::path::PathBuf::from(dir.slot(n));
        assert!(!path(0).exists());
        let farm = rig.farm(2, Flags::default());
        let snap = farm.fleet_probe().remove(0);
        farm.run_for_test(&job("x", "true"), &snap, job::Attach::Hold(Duration::from_secs(30))).unwrap();
        assert!(path(0).exists() && path(1).exists(), "the probe tries every slot's lock, which makes the file");
        assert_eq!(dir.slot(1), format!("{}/slots/1.lock", rig.root.join("farm").display()));
    }

    #[test]
    fn the_workflow_doc_names_every_verb_flag_exit_code_and_config_key() {
        let doc =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/workflow.md"))
                .unwrap();
        for action in Action::ALL {
            assert!(doc.contains(&format!("`farm {action}")), "verb {action}");
        }
        for exit in Exit::ALL {
            assert!(doc.contains(&format!("| {} | {exit} |", exit.code())), "exit {exit}");
        }
        for word in [".config/farm.toml", "[job.", "cargo install --path tools/farm --locked", "--version"] {
            assert!(doc.contains(word), "{word}");
        }
        for word in [
            "--detach",
            "--timeout",
            "--limit",
            "--host",
            "--json",
            "--verbose",
            "--tail",
            "--at",
            "--forget",
            "INK_FARM_CONFIG",
            "uses.farm/v1",
        ] {
            assert!(doc.contains(word), "{word}");
        }
    }

    #[test]
    fn the_workflow_doc_names_the_cache_flags_keys_and_the_agent_budget() {
        let doc =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/workflow.md"))
                .unwrap();
        for word in [
            "--fresh",
            "--budget",
            "inputs = [",
            "cache = false",
            "[sync]",
            "[cache]",
            "[budget]",
            "full_checks_per_day",
            "max_age_hours",
            "max_entries",
            "passed earlier on this tree",
            "wedged?",
            "Heavy-command budget",
        ] {
            assert!(doc.contains(word), "{word}");
        }
        for key in ["name", "ssh", "slots", "arch", "golden", "mold", "dir"] {
            assert!(doc.contains(&format!("\n{key} = ")), "config key {key}");
        }
    }
}
