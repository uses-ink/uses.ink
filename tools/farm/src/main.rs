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
//! - [`fleet`]: [`fleet::Member`] and [`fleet::Fleet`], generic over the transport.
//! - [`place`]: [`place::Placement`], which host gets a job.
//! - [`run`]: [`run::Farm`], the verbs. [`summary`], [`job`] and [`event`] read what comes back.
//! - [`jobs`]: the repo's job file.
//! - [`version`]: the hash of the source the binary was built from.
//! - [`workspace`]: `farm workspace`, which needs no host.

mod cli;
mod config;
mod console;
mod digest;
mod error;
mod event;
mod exit;
mod fleet;
mod host;
mod job;
mod jobs;
mod name;
mod place;
mod process;
mod program;
mod quote;
mod run;
mod snap;
mod summary;
mod transport;
mod tree;
mod version;
mod workspace;

use std::process::ExitCode;
use std::sync::Arc;

use cli::{Command, Verb};
use config::{Config, Environment};
use console::Console;
use error::{Chain, Error};
use exit::Exit;
use fleet::{Fleet, Member};
use host::{Host, Via};
use place::LeastLoaded;
use run::{Farm, Setup};
use transport::{Exec, Link, Local, Ssh};
use tree::Tree;
use workspace::Workspaces;

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
            console.out(version::Version::current());
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
                let path = Workspaces::new(&tree).forget(name)?;
                console.out(format!("workspace `{name}` forgotten and {} removed", path.display()));
                return Ok(Exit::Passed);
            }
            _ => {}
        }
        let config = Config::load(&Environment::current())?;
        let exec = Exec::new(Arc::clone(&console));
        let members = config.hosts.iter().map(|host| Self::member(host, &exec)).collect();
        let farm = Farm::new(Setup { fleet: Fleet::new(members), placement: LeastLoaded, console, flags, tree });
        farm.go(&verb)
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
    use crate::cli::{Action, Flags, Verb};
    use crate::exit::Exit;
    use crate::job::Verdict;
    use crate::name::HostName;

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
        }
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
    fn a_detached_job_outlives_the_client_and_wait_returns_its_result() {
        let rig = Rig::new("detach");
        let started = run_one(&rig, &job("bg", "sleep 1; echo finished; exit 7"), job::Attach::Detach, 1);
        assert_eq!(started.verdict, Verdict::Running);
        let member = rig.member();
        let said = member.wait(&started.id, cli::Seconds(0)).unwrap();
        assert_eq!(said.events, [event::Event::Running], "not done yet: still running, safe to call again");
        let said = member.wait(&started.id, cli::Seconds(20)).unwrap();
        assert!(matches!(said.events.first(), Some(event::Event::Done { code: 7, .. })), "{:?}", said.events);
        assert_eq!(member.log(&started.id, None).unwrap().output, ["finished"]);
        assert_eq!(member.log(&started.id, Some(1)).unwrap().output, ["finished"]);
        let missing = JobIdHack::other(&started.id);
        assert_eq!(member.wait(&missing, cli::Seconds(1)).unwrap().events, [event::Event::NoJob]);
    }

    /// A job id of the same host that no job has.
    struct JobIdHack;

    impl JobIdHack {
        fn other(id: &name::JobId) -> name::JobId {
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
        let script = |n: u32| format!("echo start{n} >> {o}; sleep 0.5; echo end{n} >> {o}", o = order.display());
        let mut ids = Vec::new();
        for n in 0..3 {
            let finished = farm.run_for_test(&job("q", &script(n)), &snap, job::Attach::Detach).unwrap();
            ids.push(finished.id);
            std::thread::sleep(Duration::from_millis(150));
        }
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
        // a held run that outlives its 1 s timeout is let go: the job survives, as `detach` says
        let finished =
            farm.run_for_test(&job("held", "sleep 3"), &snap, job::Attach::Hold(Duration::from_secs(1))).unwrap();
        assert_eq!(finished.verdict, Verdict::Running);
        let said = member.wait(&finished.id, cli::Seconds(20)).unwrap();
        assert!(matches!(said.events.first(), Some(event::Event::Done { code: 0, .. })), "{:?}", said.events);
        // the slot is free again
        let probe = member.probe().unwrap();
        assert_eq!((probe.busy, probe.queued), (0, 0));
    }

    /// Polls `check` for up to five seconds.
    fn within(mut check: impl FnMut() -> bool) -> bool {
        for _ in 0..50 {
            if check() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn a_killed_client_stops_the_job_and_frees_the_slot() {
        let rig = Rig::new("kill");
        let member = rig.member();
        let farm = rig.farm(1, Flags::default());
        let snap = farm.fleet_probe().remove(0);
        let id = name::JobId::new(HostName::local(), 1, 1);
        let job = job("held", "sleep 60");
        let start = fleet::Start { tree: &farm_tree(&rig), snap: &snap, job: &job, id: &id };
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
        assert!(matches!(log.events.first(), Some(event::Event::Done { code: 143, .. })), "{:?}", log.events);
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
        for key in ["name", "ssh", "slots", "arch", "golden", "mold", "dir"] {
            assert!(doc.contains(&format!("\n{key} = ")), "config key {key}");
        }
    }
}
