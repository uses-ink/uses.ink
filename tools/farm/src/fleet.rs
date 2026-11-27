//! The hosts, as one thing to ask. A [`Member`] is a host with the way to reach it; a [`Fleet`]
//! is all of them, probed together. Both are generic over the [`Transport`], so the logic is
//! tested against a fake and runs over [`Link`](crate::transport::Link).

use std::sync::Mutex;
use std::thread;

use crate::changes::Kind;
use crate::cli::Seconds;
use crate::console::Console;
use crate::error::Error;
use crate::event::{Event, Line, Seeded};
use crate::host::{Host, Snapshot};
use crate::job::{Attach, Job};
use crate::name::{HostName, JobId};
use crate::process::Failure;
use crate::program::{Call, Program};
use crate::transport::{Done, Pull, Push, Transport};
use crate::tree::Tree;

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

    /// Makes the workspace's tree on the host equal to the workspace here; the first time on a
    /// host it starts from the newest sibling tree by reflink. Returns how the tree came to be,
    /// when this call made it.
    pub(crate) fn sync(&self, tree: &Tree) -> Result<Option<Seeded>, Error> {
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
        self.transport.push(&Push { root: &tree.root, tree: &path, lock: &lock }).map_err(self.failed("rsync"))?;
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
        self.transport.call(call, lines).map_err(self.failed("the job"))
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
        Ok(said.events.iter().find_map(|event| if let Event::Swept(n) = event { Some(*n) } else { None }).unwrap_or(0))
    }

    /// The way to reach the host.
    #[cfg(test)]
    pub(crate) fn transport(&self) -> &T {
        &self.transport
    }
}

/// Every host.
#[derive(Debug)]
pub(crate) struct Fleet<T> {
    members: Vec<Member<T>>,
}

/// What probing every host found.
#[derive(Debug, Default)]
pub(crate) struct Probed {
    pub(crate) up: Vec<Snapshot>,
    pub(crate) down: Vec<(HostName, Error)>,
}

impl<T: Transport + Sync> Fleet<T> {
    pub(crate) fn new(members: Vec<Member<T>>) -> Self {
        Self { members }
    }

    pub(crate) fn member(&self, name: &HostName) -> Option<&Member<T>> {
        self.members.iter().find(|member| &member.host.name == name)
    }

    #[cfg(test)]
    pub(crate) fn members(&self) -> &[Member<T>] {
        &self.members
    }

    /// Probes every host at once; a host that does not answer is reported down and left out.
    pub(crate) fn probe(&self, console: &Console) -> Probed {
        let results: Vec<_> = thread::scope(|scope| {
            let running: Vec<_> = self.members.iter().map(|member| scope.spawn(move || member.probe())).collect();
            running.into_iter().zip(&self.members).map(|(handle, member)| (member, handle.join())).collect()
        });
        let mut probed = Probed::default();
        for (member, joined) in results {
            match joined {
                Ok(Ok(snap)) => probed.up.push(snap),
                Ok(Err(error)) => probed.down.push((member.host.name.clone(), error)),
                Err(_) => console.note(format!("probing `{}` panicked", member.host.name)),
            }
        }
        probed
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A transport that answers from a script and remembers what it was asked.

    use std::collections::VecDeque;
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::ExitStatus;
    use std::sync::Mutex;

    use super::*;

    /// One scripted answer to a call.
    #[derive(Debug, Clone)]
    pub(crate) struct Reply {
        pub(crate) lines: Vec<String>,
        pub(crate) code: i32,
    }

    impl Reply {
        pub(crate) fn ok(lines: &[&str]) -> Self {
            Self { lines: lines.iter().map(|line| (*line).to_owned()).collect(), code: 0 }
        }
    }

    /// Answers calls from a queue, in order, and records them.
    #[derive(Debug, Default)]
    pub(crate) struct Fake {
        pub(crate) replies: Mutex<VecDeque<Reply>>,
        pub(crate) calls: Mutex<Vec<Call>>,
        pub(crate) pushed: Mutex<Vec<String>>,
        pub(crate) pulled: Mutex<Vec<String>>,
        pub(crate) local: bool,
    }

    impl Fake {
        pub(crate) fn saying(replies: Vec<Reply>) -> Self {
            Self { replies: Mutex::new(replies.into()), ..Self::default() }
        }
    }

    impl Transport for Fake {
        fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
            self.calls.lock().unwrap().push(call.clone());
            let reply = self.replies.lock().unwrap().pop_front().unwrap_or(Reply { lines: vec![], code: 0 });
            reply.lines.iter().for_each(|line| lines(line));
            Ok(Done { status: ExitStatus::from_raw(reply.code << 8), stderr: String::new() })
        }

        fn push(&self, push: &Push<'_>) -> Result<(), Failure> {
            self.pushed.lock().unwrap().push(push.tree.to_owned());
            Ok(())
        }

        fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure> {
            self.pulled.lock().unwrap().push(pull.files.to_owned());
            Ok(())
        }

        fn is_local(&self) -> bool {
            self.local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Fake, Reply};
    use super::*;
    use crate::host::Via;
    use crate::place::Need;

    fn remote(name: &str) -> Host {
        Host { name: name.parse().unwrap(), via: Via::Ssh("x".parse().unwrap()), ..Host::local() }
    }

    fn tree() -> Tree {
        Tree { root: "/w/farm".into(), repo: "uses.ink".parse().unwrap(), workspace: "farm".parse().unwrap() }
    }

    const PROBE: &[&str] =
        &["now 1", "arch x86_64", "cores 12", "load 0.5", "slots 2", "slot 0 free", "slot 1 busy", "queued 0"];

    #[test]
    fn a_probe_call_asks_with_the_dir_and_the_configured_slots() {
        let member = Member::new(Host { slots: Some(3), ..remote("petal") }, Fake::saying(vec![Reply::ok(PROBE)]));
        let snap = member.probe().unwrap();
        assert_eq!((snap.cores, snap.busy), (12, 1));
        let calls = member.transport.calls.lock().unwrap();
        assert_eq!(calls.first().unwrap().program, Program::Probe);
        assert_eq!(calls.first().unwrap().args, ["farm", "3"]);
    }

    #[test]
    fn the_first_sync_seeds_then_pushes_and_the_second_does_nothing() {
        let member = Member::new(remote("petal"), Fake::saying(vec![Reply::ok(&["@@farm:seed reflink main"])]));
        assert_eq!(member.sync(&tree()).unwrap(), Some(Seeded::Reflink("main".to_owned())));
        assert_eq!(member.sync(&tree()).unwrap(), None);
        assert_eq!(*member.transport.pushed.lock().unwrap(), ["farm/uses.ink/farm"]);
        assert_eq!(member.transport.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_local_member_is_never_synced() {
        let member = Member::new(Host::local(), Fake { local: true, ..Fake::default() });
        assert_eq!(member.sync(&tree()).unwrap(), None);
        assert!(member.transport.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failing_call_is_an_error_naming_the_host() {
        let member = Member::new(remote("petal"), Fake::saying(vec![Reply { lines: vec![], code: 255 }]));
        let error = member.probe().unwrap_err();
        assert!(error.to_string().contains("`petal`"), "{error}");
    }

    #[test]
    fn the_launch_passes_every_value_as_its_own_argument_and_the_command_after_env() {
        let member = Member::new(Host { mold: true, ..remote("petal") }, Fake::default());
        let snap = Snapshot::read(&member.host, &PROBE.join("\n")).unwrap();
        let job = Job {
            label: "x".to_owned(),
            words: ["cargo", "test", "-E", "test(a b;c)"].map(str::to_owned).to_vec(),
            need: Need::Any,
            env: vec![],
            limit: None,
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
    fn a_held_launch_holds_stdin_for_the_timeout() {
        let member = Member::new(remote("petal"), Fake::default());
        let snap = Snapshot::read(&member.host, &PROBE.join("\n")).unwrap();
        let job =
            Job { label: "x".to_owned(), words: vec!["cargo".to_owned()], need: Need::Any, env: vec![], limit: None };
        let id = JobId::new(member.host.name.clone(), 1, 1);
        let start = Start { tree: &tree(), snap: &snap, job: &job, id: &id };
        let call = member.launch(&start, Attach::Hold(std::time::Duration::from_secs(540)), Seconds(1500));
        assert_eq!(call.stdin, crate::program::Stdin::Held(std::time::Duration::from_secs(540)));
        assert_eq!(call.args.first().map(String::as_str), Some("held"));
    }

    #[test]
    fn probing_the_fleet_reports_the_host_that_is_down_and_keeps_the_rest() {
        let up = Member::new(remote("up"), Fake::saying(vec![Reply::ok(PROBE)]));
        let down = Member::new(remote("down"), Fake::saying(vec![Reply { lines: vec![], code: 255 }]));
        let probed = Fleet::new(vec![up, down]).probe(&Console::default());
        assert_eq!(probed.up.len(), 1);
        assert_eq!(probed.down.first().map(|(name, _)| name.as_str()), Some("down"));
    }
}
