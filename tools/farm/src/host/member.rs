//! A [`Member`] is a host with the way to reach it, generic over the [`Transport`], so the logic is
//! tested against a fake and runs over [`Link`](crate::host::transport::Link).

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::Seconds;
use crate::cli::error::Error;
use crate::cli::name::{Excluded, JobId};
use crate::config::tree::Tree;
use crate::host::transport::process::Failure;
use crate::host::transport::program::{Call, Program};
use crate::host::transport::{Done, Pull, Push, Transport};
use crate::host::{Host, Snapshot};
use crate::job::event::{Event, Line, Seeded};
use crate::job::{Attach, Job};
use crate::place::wake::Fault as WakeFault;
use crate::run::changes::Kind;

/// What a quick call printed: its events, and every other line.
#[derive(Debug, Default)]
pub(crate) struct Said {
    pub(crate) events: Vec<Event>,
    pub(crate) output: Vec<String>,
}

/// A job to start on a member.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Start<'a> {
    pub(crate) tree: &'a Tree,
    pub(crate) snap: &'a Snapshot,
    pub(crate) job: &'a Job,
    pub(crate) id: &'a JobId,
}

/// A host and the way to reach it.
#[derive(Debug)]
pub(crate) struct Member<T> {
    pub(crate) host: Host,
    transport: T,
    /// Whether this run has synced the workspace here; held while it syncs, so a second job
    /// bound for the same host waits for the first sync instead of repeating it.
    synced: Mutex<bool>,
}

impl<T: Transport> Member<T> {
    /// How often a waking host is probed.
    const POLL: Duration = Duration::from_secs(5);

    pub(crate) fn new(host: Host, transport: T) -> Self {
        Self { host, transport, synced: Mutex::new(false) }
    }

    fn failed(&self, what: &'static str) -> impl FnOnce(Failure) -> Error + '_ {
        move |source| Error::Call { host: self.host.name.clone(), what, source }
    }

    /// Runs a program that ends on its own and returns what it said.
    pub(crate) fn say(&self, call: &Call, what: &'static str) -> Result<Said, Error> {
        let mut said = Said::default();
        let mut fault = None;
        let done = self
            .transport
            .call(call, &mut |text| match Line::read(text) {
                Ok(Line::Event(event)) => said.events.push(event),
                Ok(Line::Output(line)) => said.output.push(line),
                Err(error) => fault = fault.take().or(Some(error)),
            })
            .and_then(Done::ok)
            .map_err(self.failed(what))?;
        drop(done);
        match fault {
            Some(source) => Err(Error::Said { host: self.host.name.clone(), source }),
            None => Ok(said),
        }
    }

    /// What the host is doing now.
    pub(crate) fn probe(&self) -> Result<Snapshot, Error> {
        let slots = self.host.slots.map_or_else(String::new, |slots| slots.to_string());
        let call = Call::new(Program::Probe, vec![self.host.dir.as_str().to_owned(), slots]);
        let said = self.say(&call, "the probe")?;
        Snapshot::read(&self.host, &said.output.join("\n"))
            .map_err(|source| Error::Probe { host: self.host.name.clone(), source })
    }

    /// Starts the sleeping host with its wake command and waits for it to answer a probe, both
    /// within the host's wake timeout; the snapshot it answers with.
    pub(crate) fn wake(&self) -> Result<Snapshot, Error> {
        let (host, wake) = (&self.host.name, self.host.wake.as_ref());
        let failed = |source| Error::Wake { host: host.clone(), source };
        let wake = wake.ok_or_else(|| failed(WakeFault::Blank))?;
        let deadline = Instant::now() + wake.timeout.duration();
        wake.run(host, deadline).map_err(failed)?;
        loop {
            let last = match self.probe() {
                Ok(snap) => return Ok(snap),
                Err(last) => last,
            };
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                let (command, secs) = (wake.shown(host), wake.timeout.0);
                return Err(failed(WakeFault::Silent { command, secs, last: Box::new(last) }));
            }
            thread::sleep(left.min(Self::POLL));
        }
    }

    /// Makes the workspace's tree on the host equal to the workspace here; the first time on a
    /// host it starts from the newest sibling tree by reflink. Returns how the tree came to be,
    /// when this call made it.
    pub(crate) fn sync(&self, tree: &Tree, exclude: &[Excluded]) -> Result<Option<Seeded>, Error> {
        if self.transport.is_local() {
            return Ok(None);
        }
        let mut synced = self.synced.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *synced {
            return Ok(None);
        }
        let args = [self.host.dir.as_str(), tree.repo.as_str(), tree.workspace.as_str()].map(str::to_owned).to_vec();
        let said = self.say(&Call::new(Program::Seed, args), "seeding the tree")?;
        let seeded = said.events.into_iter().find_map(|event| match event {
            Event::Seed(seeded) => Some(seeded),
            _ => None,
        });
        let seeded =
            seeded.ok_or_else(|| Error::Silent { host: self.host.name.clone(), what: "how the tree was seeded" })?;
        let (path, lock) = (tree.on(&self.host), tree.lock(&self.host));
        self.transport
            .push(&Push { root: &tree.root, tree: &path, lock: &lock, exclude })
            .map_err(self.failed("rsync"))?;
        *synced = true;
        Ok(Some(seeded))
    }

    /// The call that runs `start.job`: the launcher with the body's text, its parameters, then
    /// the command under `env`.
    pub(crate) fn launch(&self, start: &Start<'_>, attach: Attach, limit: Seconds) -> Call {
        let Start { tree, snap, job, id } = *start;
        let mode = match attach {
            Attach::Hold(_) => "held",
            Attach::Detach => "detach",
        };
        let mut args: Vec<String> = [
            mode,
            Program::Body.text(),
            self.host.dir.as_str(),
            &id.dir(),
            &snap.slots.to_string(),
            &limit.0.to_string(),
            &tree.on(&self.host),
            if self.host.mold { "mold" } else { "plain" },
            &job.shown(),
            "env",
        ]
        .map(str::to_owned)
        .to_vec();
        args.extend(job.vars(snap));
        args.extend(job.words.iter().cloned());
        let call = Call::new(Program::Run, args);
        match attach {
            Attach::Hold(until) => call.held(until),
            Attach::Detach => call,
        }
    }

    /// Runs a started job's launcher, each line into `lines`.
    pub(crate) fn run(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Error> {
        self.transport.call(call, lines).map_err(|source| match source {
            Failure::Cancelled => Error::Cancelled { host: self.host.name.clone() },
            source => Error::Call { host: self.host.name.clone(), what: "the job", source },
        })
    }

    /// Blocks until the job is done or `limit` seconds pass.
    pub(crate) fn wait(&self, id: &JobId, limit: Seconds) -> Result<Said, Error> {
        let args = [self.host.dir.as_str(), &id.dir(), &limit.0.to_string()].map(str::to_owned).to_vec();
        self.say(&Call::new(Program::Wait, args), "waiting")
    }

    /// The job's log on the host, whole or the last `tail` lines.
    pub(crate) fn log(&self, id: &JobId, tail: Option<u32>) -> Result<Said, Error> {
        let args = [self.host.dir.as_str(), &id.dir(), &tail.map_or_else(String::new, |n| n.to_string())]
            .map(str::to_owned)
            .to_vec();
        self.say(&Call::new(Program::Log, args), "reading the log")
    }

    /// The files of `kind` the job changed, relative to the tree.
    pub(crate) fn changed(&self, tree: &Tree, id: &JobId, kind: Kind) -> Result<Said, Error> {
        let args =
            [self.host.dir.as_str(), &id.dir(), &tree.on(&self.host), &kind.pattern()].map(str::to_owned).to_vec();
        self.say(&Call::new(Program::Changed, args), "listing changed files")
    }

    /// Whether the host runs in the workspace itself, so there is nothing to bring back.
    pub(crate) fn is_local(&self) -> bool {
        self.transport.is_local()
    }

    /// Brings `files` (relative paths, one per line) from the host's tree into the workspace.
    pub(crate) fn pull(&self, tree: &Tree, files: &str) -> Result<(), Error> {
        let path = tree.on(&self.host);
        self.transport.pull(&Pull { root: &tree.root, tree: &path, files }).map_err(self.failed("rsync"))
    }

    /// Removes the state of jobs older than seven days; how many.
    pub(crate) fn sweep(&self) -> Result<u32, Error> {
        let said = self.say(&Call::new(Program::Sweep, vec![self.host.dir.as_str().to_owned()]), "sweeping")?;
        said.events
            .iter()
            .find_map(|event| if let Event::Swept(n) = event { Some(*n) } else { None })
            .ok_or_else(|| Error::Silent { host: self.host.name.clone(), what: "how many it swept" })
    }

    /// The way to reach the host.
    #[cfg(test)]
    pub(crate) fn transport(&self) -> &T {
        &self.transport
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::console::Console;
    use crate::host::Fleet;
    use crate::host::fake::{Fake, Reply};
    use crate::place::Need;

    fn tree() -> Tree {
        Tree { root: "/w/farm".into(), repo: "uses.ink".parse().unwrap(), workspace: "farm".parse().unwrap() }
    }

    #[test]
    fn a_probe_call_asks_with_the_dir_and_the_configured_slots() {
        let member =
            Member::new(Host { slots: Some(3), ..Fake::remote("petal") }, Fake::saying(vec![Reply::ok(Reply::PROBE)]));
        let snap = member.probe().unwrap();
        assert_eq!((snap.cores, snap.busy), (12, 1));
        let calls = member.transport.calls.lock().unwrap();
        assert_eq!(calls.first().unwrap().program, Program::Probe);
        assert_eq!(calls.first().unwrap().args, ["farm", "3"]);
    }

    #[test]
    fn the_first_sync_seeds_then_pushes_and_the_second_does_nothing() {
        let member = Member::new(Fake::remote("petal"), Fake::saying(vec![Reply::ok(&["@@farm:seed reflink main"])]));
        assert_eq!(member.sync(&tree(), &[]).unwrap(), Some(Seeded::Reflink("main".to_owned())));
        assert_eq!(member.sync(&tree(), &[]).unwrap(), None);
        assert_eq!(*member.transport.pushed.lock().unwrap(), ["farm/uses.ink/farm"]);
        assert_eq!(member.transport.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_local_member_is_never_synced() {
        let member = Member::new(Host::local(), Fake { local: true, ..Fake::default() });
        assert_eq!(member.sync(&tree(), &[]).unwrap(), None);
        assert!(member.transport.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failing_call_is_an_error_naming_the_host() {
        let member = Member::new(Fake::remote("petal"), Fake::saying(vec![Reply { lines: vec![], code: 255 }]));
        let error = member.probe().unwrap_err();
        assert!(error.to_string().contains("`petal`"), "{error}");
    }

    #[test]
    fn the_launch_passes_every_value_as_its_own_argument_and_the_command_after_env() {
        let member = Member::new(Host { mold: true, ..Fake::remote("petal") }, Fake::default());
        let snap = Snapshot::read(&member.host, &Reply::PROBE.join("\n")).unwrap();
        let job = Job {
            label: "x".to_owned(),
            words: ["cargo", "test", "-E", "test(a b;c)"].map(str::to_owned).to_vec(),
            need: Need::Any,
            env: vec![],
            reuse: crate::job::Reuse::Never,
            limit: None,
            ceiling: None,
            key: crate::place::history::Key::stage("x"),
        };
        let id = JobId::new(member.host.name.clone(), 255, 1);
        let call =
            member.launch(&Start { tree: &tree(), snap: &snap, job: &job, id: &id }, Attach::Detach, Seconds(1500));
        assert_eq!(call.program, Program::Run);
        let args = &call.args;
        assert_eq!(args.first().map(String::as_str), Some("detach"));
        assert_eq!(args.get(1).map(String::as_str), Some(Program::Body.text()));
        assert_eq!(
            &args[2..9],
            ["farm", "petal.ff1", "2", "1500", "farm/uses.ink/farm", "mold", "cargo test -E test(a b;c)"]
        );
        assert_eq!(args.get(9).map(String::as_str), Some("env"));
        assert_eq!(args.last().map(String::as_str), Some("test(a b;c)"));
        assert!(args.contains(&"CARGO_BUILD_JOBS=6".to_owned()));
    }

    #[test]
    fn a_double_dash_and_what_follows_are_in_the_argv_the_transport_sends() {
        let member = Member::new(Fake::remote("petal"), Fake::default());
        let snap = Snapshot::read(&member.host, &Reply::PROBE.join("\n")).unwrap();
        let parsed = crate::cli::Command::parse(
            "cargo clippy -p x --all-targets -- -D warnings".split(' ').map(str::to_owned).collect(),
        )
        .unwrap();
        let crate::cli::Verb::Cargo(args) = parsed.verb else { panic!("not a cargo verb") };
        let job = Job::cargo(&args);
        let id = JobId::new(member.host.name.clone(), 255, 1);
        let call =
            member.launch(&Start { tree: &tree(), snap: &snap, job: &job, id: &id }, Attach::Detach, Seconds(1500));
        let sent = &call.args;
        let env = sent.iter().position(|arg| arg == "env").unwrap();
        let after: Vec<&str> = sent.iter().skip(env).map(String::as_str).collect();
        assert_eq!(
            &after[after.len() - 8..],
            ["cargo", "clippy", "-p", "x", "--all-targets", "--", "-D", "warnings"],
            "every word is its own argument and `--` stays"
        );
    }

    /// A member whose wake command is a script that appends its arguments to `marker`.
    fn sleeper(dir: &std::path::Path, body: &str, replies: Vec<Reply>) -> Member<Fake> {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::create_dir_all(dir).unwrap();
        let script = dir.join("wake.sh");
        std::fs::write(&script, format!("#!/bin/sh\necho \"$@\" >> {}/marker\n{body}\n", dir.display())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let wake = crate::place::wake::Wake::new(&script.display().to_string(), Some(Seconds(1)), None);
        Member::new(Host { wake, ..Fake::remote("farm-a") }, Fake::saying(replies))
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("farm-fleet-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn waking_runs_the_command_with_the_host_name_then_probes_until_it_answers() {
        let dir = scratch("wake");
        let member = sleeper(&dir, "true", vec![Reply { lines: vec![], code: 255 }, Reply::ok(Reply::PROBE)]);
        let snap = member.wake().unwrap();
        assert_eq!(snap.name.as_str(), "farm-a");
        assert_eq!(std::fs::read_to_string(dir.join("marker")).unwrap().trim(), "farm-a");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_wake_that_fails_or_whose_host_stays_silent_is_a_typed_error_not_a_hang() {
        let dir = scratch("fail");
        let failing = sleeper(&dir, "exit 7", vec![]);
        let error = failing.wake().unwrap_err();
        let text = crate::cli::error::Chain(&error).to_string();
        assert!(matches!(error, Error::Wake { .. }) && text.contains("exit status: 7"), "{text}");
        let silent = Reply { lines: vec![], code: 255 };
        let quiet = sleeper(&dir, "true", vec![silent.clone(), silent.clone(), silent]);
        let started = std::time::Instant::now();
        let error = quiet.wake().unwrap_err();
        let text = crate::cli::error::Chain(&error).to_string();
        assert!(text.contains("still did not answer within 1 s"), "{text}");
        assert!(started.elapsed() < std::time::Duration::from_secs(8));
        let plain = Member::new(Fake::remote("petal"), Fake::default());
        assert!(matches!(plain.wake(), Err(Error::Wake { source: crate::place::wake::Fault::Blank, .. })));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_host_with_a_wake_command_that_does_not_answer_is_asleep_and_one_without_is_down() {
        let dir = scratch("probe");
        let silent = Reply { lines: vec![], code: 255 };
        let asleep = sleeper(&dir, "true", vec![silent.clone()]);
        let broken = Member::new(Fake::remote("broken"), Fake::saying(vec![silent]));
        let up = Member::new(Fake::remote("up"), Fake::saying(vec![Reply::ok(Reply::PROBE)]));
        let probed = Fleet::new(vec![asleep, broken, up]).probe(&Console::default());
        assert_eq!(probed.asleep.iter().map(|host| host.name.as_str()).collect::<Vec<_>>(), ["farm-a"]);
        assert_eq!(probed.down.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), ["broken"]);
        assert_eq!(probed.up.len(), 1);
        assert!(!dir.join("marker").exists(), "probing never wakes");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_held_launch_holds_stdin_for_the_timeout() {
        let member = Member::new(Fake::remote("petal"), Fake::default());
        let snap = Snapshot::read(&member.host, &Reply::PROBE.join("\n")).unwrap();
        let job = Job {
            label: "x".to_owned(),
            words: vec!["cargo".to_owned()],
            need: Need::Any,
            env: vec![],
            limit: None,
            ceiling: None,
            key: crate::place::history::Key::stage("x"),
            reuse: crate::job::Reuse::Never,
        };
        let id = JobId::new(member.host.name.clone(), 1, 1);
        let start = Start { tree: &tree(), snap: &snap, job: &job, id: &id };
        let call = member.launch(&start, Attach::Hold(std::time::Duration::from_secs(540)), Seconds(1500));
        assert_eq!(call.stdin, crate::host::transport::program::Stdin::Held(std::time::Duration::from_secs(540)));
        assert_eq!(call.args.first().map(String::as_str), Some("held"));
    }
}
