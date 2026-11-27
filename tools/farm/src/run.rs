//! The verbs that use the hosts: [`Farm`] places jobs, runs them, and reports.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::changes::{Changes, Kind};
use crate::cli::{Action, Flags, Seconds, Usage, Verb};
use crate::console::Console;
use crate::error::{Chain, Error};
use crate::event::{Event, Seeded};
use crate::exit::Exit;
use crate::fleet::{Fleet, Start};
use crate::host::Snapshot;
use crate::job::{Attach, Document, Finished, Flow, Job, LogFile, Verdict};
use crate::jobs::Jobs;
use crate::name::{HostName, JobId, JobName};
use crate::place::{Need, Placement};
use crate::summary::Summary;
use crate::transport::Transport;
use crate::tree::Tree;
use crate::version::{Freshness, Version};

/// Where a job's log is kept here, below the workspace: `target/farm/<job>.log`.
const LOGS: [&str; 2] = ["target", "farm"];

/// What a verb produced: the jobs that ran, the farm's own failures, and the files a bless or a
/// fmt brought back.
#[derive(Debug, Default)]
pub(crate) struct Batch {
    pub(crate) done: Vec<Finished>,
    pub(crate) failed: Vec<Error>,
    pub(crate) blessed: Vec<String>,
    pub(crate) formatted: Vec<String>,
}

/// What a [`Farm`] is made of.
#[derive(Debug)]
pub(crate) struct Setup<T, P> {
    pub(crate) fleet: Fleet<T>,
    pub(crate) placement: P,
    pub(crate) console: Arc<Console>,
    pub(crate) flags: Flags,
    pub(crate) tree: Tree,
}

/// The farm: the hosts, the placement, the workspace, and what the person asked for.
#[derive(Debug)]
pub(crate) struct Farm<T, P> {
    fleet: Fleet<T>,
    placement: P,
    console: Arc<Console>,
    flags: Flags,
    tree: Tree,
    salt: AtomicU32,
}

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    pub(crate) fn new(setup: Setup<T, P>) -> Self {
        let Setup { fleet, placement, console, flags, tree } = setup;
        Self { fleet, placement, console, flags, tree, salt: AtomicU32::new(0) }
    }

    /// Runs a verb, prints its report and returns how it ended.
    pub(crate) fn go(&self, verb: &Verb) -> Result<Exit, Error> {
        let batch = match verb {
            Verb::Check(names) => self.check(names)?,
            Verb::Cargo(args) => self.cargo(args)?,
            Verb::Bless(args) => self.bless(args)?,
            Verb::Fmt => self.fmt()?,
            Verb::Wait(id) => self.wait(id)?,
            Verb::Status => return self.status(),
            Verb::Log(id) => return self.log(id),
            Verb::Make(_) | Verb::Forget(_) | Verb::Version => return Err(Usage::NoVerb.into()),
        };
        Ok(self.report(&batch))
    }

    /// Places `needs` on hosts at once: each is reserved a slot or a place in the queue before
    /// the next is placed, so a fan-out spreads over the hosts.
    fn place(&self, needs: &[Need]) -> Result<Vec<Snapshot>, Error> {
        let probed = self.fleet.probe(&self.console);
        for (host, error) in &probed.down {
            self.console.note(format!("`{host}` is down and left out: {}", Chain(error)));
        }
        let mut board = probed.up;
        let mut chosen = Vec::new();
        for need in needs {
            let at = self.placement.choose(*need, self.flags.host.as_ref(), &board)?;
            if let Some(snap) = board.get_mut(at) {
                chosen.push(snap.clone());
                snap.reserve();
            }
        }
        Ok(chosen)
    }

    /// Probes the hosts and returns those that answered.
    #[cfg(test)]
    pub(crate) fn fleet_probe(&self) -> Vec<Snapshot> {
        self.fleet.probe(&self.console).up
    }

    /// Runs the jobs of a check without printing a report.
    #[cfg(test)]
    pub(crate) fn go_for_test(&self, verb: &Verb) -> Result<Batch, Error> {
        match verb {
            Verb::Check(names) => self.check(names),
            _ => Err(Usage::NoVerb.into()),
        }
    }

    /// Runs one job on a chosen host.
    #[cfg(test)]
    pub(crate) fn run_for_test(&self, job: &Job, snap: &Snapshot, attach: Attach) -> Result<Finished, Error> {
        self.run(job, snap, attach)
    }

    fn next(&self, host: &HostName) -> JobId {
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis());
        JobId::new(host.clone(), millis, self.salt.fetch_add(1, Ordering::Relaxed))
    }

    fn attach(&self) -> Attach {
        if self.flags.detach { Attach::Detach } else { Attach::Hold(self.flags.timeout.duration()) }
    }

    /// Says how a host's new tree came to be, when it did.
    fn seeded(&self, host: &HostName, seeded: Option<Seeded>) {
        match seeded {
            Some(Seeded::Copy(from)) => {
                self.console
                    .note(format!("`{host}` has no reflinks: its tree was copied in full from `{from}` (slow once)"));
            }
            Some(Seeded::Reflink(from)) => self.console.note(format!("`{host}`: new tree seeded from `{from}`")),
            Some(Seeded::Present | Seeded::Empty) | None => {}
        }
    }

    /// Says where a job is about to run.
    fn announce(&self, job: &Job, snap: &Snapshot) {
        self.console.note(format!(
            "{}: on `{}`, {} of {} slots busy, {} queued",
            job.label, snap.name, snap.busy, snap.slots, snap.queued
        ));
    }

    /// The hard limit of a job: the person's `--limit`, else the job's own, else the default.
    fn limit(&self, job: &Job) -> Seconds {
        self.flags.limit.or(job.limit).unwrap_or(Seconds::LIMIT)
    }

    /// Runs one job on the host `snap` describes: sync, take a slot, run, summarise.
    fn run(&self, job: &Job, snap: &Snapshot, attach: Attach) -> Result<Finished, Error> {
        let member = self
            .fleet
            .member(&snap.name)
            .ok_or_else(|| Error::NoHost { need: job.need, up: vec![snap.name.clone()] })?;
        let id = self.next(&snap.name);
        self.announce(job, snap);
        self.seeded(&snap.name, member.sync(&self.tree)?);
        let log = self.tree.root.join(LOGS[0]).join(LOGS[1]).join(format!("{id}.log"));
        let file = LogFile::create(&log).map_err(|source| Error::Io { path: log.clone(), source })?;
        let mut flow = Flow { file: Some(file), ..Flow::default() };
        let call = member.launch(&Start { tree: &self.tree, snap, job, id: &id }, attach, self.limit(job));
        let eta = snap.eta().map_or_else(|| "unknown".to_owned(), |eta| format!("about {} s", eta.as_secs()));
        member.run(&call, &mut |text| {
            if let Some(Event::Queued { behind }) = flow.feed(text) {
                self.console.note(format!("{}: queued behind {behind}, host `{}` free in {eta}", job.label, snap.name));
            }
        })?;
        if let Some(source) = flow.fault.take() {
            return Err(Error::Said { host: snap.name.clone(), source });
        }
        if let Some(source) = flow.log_error() {
            return Err(Error::Io { path: log, source });
        }
        if flow.nobody {
            return Err(Error::NoBody { job: id });
        }
        let verdict = flow.verdict();
        if verdict == Verdict::Lost {
            return Err(Error::Lost { job: id });
        }
        Ok(Finished {
            summary: Summary::of(&flow.log),
            id,
            label: job.label.clone(),
            command: job.shown(),
            host: snap.name.clone(),
            slot: flow.slot,
            queued: flow.queued,
            verdict,
            log,
        })
    }

    /// Runs the jobs at once, each on its placed host; the results come back in order.
    fn fan(&self, jobs: &[Job], snaps: &[Snapshot]) -> Batch {
        let attach = self.attach();
        let results: Vec<Result<Finished, Error>> = thread::scope(|scope| {
            let running: Vec<_> =
                jobs.iter().zip(snaps).map(|(job, snap)| scope.spawn(move || self.run(job, snap, attach))).collect();
            running
                .into_iter()
                .zip(jobs)
                .map(|(handle, job)| handle.join().unwrap_or_else(|_| Err(Error::Crashed { label: job.label.clone() })))
                .collect()
        });
        let mut batch = Batch::default();
        for result in results {
            match result {
                Ok(done) => batch.done.push(done),
                Err(error) => batch.failed.push(error),
            }
        }
        batch
    }

    /// The repo's declared jobs, one per job, all placed at once: the wall time is the slowest
    /// job, not the sum.
    fn check(&self, names: &[JobName]) -> Result<Batch, Error> {
        let jobs: Vec<Job> = Jobs::load(&self.tree.root)?.pick(names)?.into_iter().map(Job::from).collect();
        let needs: Vec<Need> = jobs.iter().map(|job| job.need).collect();
        let snaps = self.place(&needs)?;
        Ok(self.fan(&jobs, &snaps))
    }

    /// Any cargo command, on a placed host.
    fn cargo(&self, args: &[String]) -> Result<Batch, Error> {
        let mut words = vec!["cargo".to_owned()];
        words.extend(args.iter().cloned());
        let job = Job { label: words.join(" "), need: Need::of_cargo(args), words, env: Vec::new(), limit: None };
        let snaps = self.place(&[job.need])?;
        Ok(self.fan(&[job], &snaps))
    }

    /// Nextest with `INSTA_UPDATE=always` on a golden host, then only the changed `.snap` files
    /// come back. A snapshot edited here after the sync started is never overwritten, as in `fmt`.
    fn bless(&self, args: &[String]) -> Result<Batch, Error> {
        if self.flags.detach {
            return Err(Usage::Extra { verb: Action::Bless, given: "--detach".to_owned() }.into());
        }
        let mut words: Vec<String> = ["cargo", "nextest", "run", "--locked"].map(str::to_owned).to_vec();
        words.extend(args.iter().cloned());
        let env = ["INSTA_UPDATE=always", "INSTA_FORCE_PASS=1"].map(str::to_owned).to_vec();
        let job = Job { label: "bless".to_owned(), words, need: Need::Golden, env, limit: None };
        let snaps = self.place(&[Need::Golden])?;
        let started = SystemTime::now();
        let mut batch = self.fan(std::slice::from_ref(&job), &snaps);
        if let Some(done) = batch.done.first().filter(|done| matches!(done.verdict, Verdict::Done { .. })) {
            match self.fetch(done, Kind::Snap, Some(started)) {
                Ok(snaps) => batch.blessed = snaps.paths().to_vec(),
                Err(error) => batch.failed.push(error),
            }
        }
        Ok(batch)
    }

    /// `cargo fmt --all` on any host, then only the `.rs` files whose bytes changed come back.
    /// A file edited here after the sync started is never overwritten: nothing comes back.
    fn fmt(&self) -> Result<Batch, Error> {
        if self.flags.detach {
            return Err(Usage::Extra { verb: Action::Fmt, given: "--detach".to_owned() }.into());
        }
        let words: Vec<String> = ["cargo", "fmt", "--all"].map(str::to_owned).to_vec();
        let job = Job { label: "fmt".to_owned(), words, need: Need::Any, env: Vec::new(), limit: None };
        let snaps = self.place(&[Need::Any])?;
        let started = SystemTime::now();
        let mut batch = self.fan(std::slice::from_ref(&job), &snaps);
        if let Some(done) = batch.done.first().filter(|done| matches!(done.verdict, Verdict::Done { code: 0, .. })) {
            match self.fetch(done, Kind::Rust, Some(started)) {
                Ok(changes) => batch.formatted = changes.paths().to_vec(),
                Err(error) => batch.failed.push(error),
            }
        }
        Ok(batch)
    }

    /// Brings back the files of `kind` a job changed; with `since`, only if none of them was
    /// edited here after that time.
    fn fetch(&self, done: &Finished, kind: Kind, since: Option<SystemTime>) -> Result<Changes, Error> {
        let member = self
            .fleet
            .member(&done.host)
            .ok_or_else(|| Error::Silent { host: done.host.clone(), what: "anything: it left the fleet" })?;
        let said = member.changed(&self.tree, &done.id, kind)?;
        let changes =
            Changes::read(kind, &said.output).map_err(|source| Error::Changes { host: done.host.clone(), source })?;
        if changes.is_empty() {
            return Ok(changes);
        }
        if let Some(since) = since.filter(|_| !member.is_local()) {
            let paths = changes
                .edited(&self.tree.root, since)
                .map_err(|source| Error::Io { path: self.tree.root.clone(), source })?;
            if !paths.is_empty() {
                return Err(Error::Edited { host: done.host.clone(), paths });
            }
        }
        member.pull(&self.tree, &changes.list())?;
        Ok(changes)
    }

    /// Blocks until a detached job is done, or the timeout passes.
    fn wait(&self, id: &JobId) -> Result<Batch, Error> {
        let member = self.fleet.member(&id.host).ok_or_else(|| Error::NoHost { need: Need::Any, up: Vec::new() })?;
        let said = member.wait(id, self.flags.timeout)?;
        let mut batch = Batch::default();
        let finished = |verdict, summary, log| Finished {
            id: id.clone(),
            label: "wait".to_owned(),
            command: "started earlier; see `farm log`".to_owned(),
            host: id.host.clone(),
            slot: None,
            queued: Duration::ZERO,
            verdict,
            log,
            summary,
        };
        let path = self.tree.root.join(LOGS[0]).join(LOGS[1]).join(format!("{id}.log"));
        match said.events.first() {
            Some(Event::Done { code, ran }) => {
                let text = member.log(id, None)?.output.join("\n");
                std::fs::create_dir_all(path.parent().unwrap_or(&self.tree.root))
                    .and_then(|()| std::fs::write(&path, &text))
                    .map_err(|source| Error::Io { path: path.clone(), source })?;
                batch.done.push(finished(Verdict::Done { code: *code, ran: *ran }, Summary::of(&text), path));
            }
            Some(Event::Running) => batch.done.push(finished(Verdict::Running, Summary::default(), path)),
            Some(Event::NoJob) => return Err(Error::NoJob { job: id.clone() }),
            Some(Event::Lost) | None => return Err(Error::Lost { job: id.clone() }),
            Some(_) => return Err(Error::Silent { host: id.host.clone(), what: "how the job ended" }),
        }
        Ok(batch)
    }

    /// Prints a job's log from its host.
    fn log(&self, id: &JobId) -> Result<Exit, Error> {
        let member = self.fleet.member(&id.host).ok_or_else(|| Error::NoHost { need: Need::Any, up: Vec::new() })?;
        let said = member.log(id, self.flags.tail)?;
        if said.events.contains(&Event::NoJob) {
            return Err(Error::NoJob { job: id.clone() });
        }
        said.output.iter().for_each(|line| self.console.out(line));
        Ok(Exit::Passed)
    }

    /// Every host: arch, load, slots, queue, jobs; and the old job state swept.
    fn status(&self) -> Result<Exit, Error> {
        if self.flags.json {
            return Err(Usage::Extra { verb: Action::Status, given: "--json".to_owned() }.into());
        }
        match Version::current().against(&self.tree.root) {
            Freshness::Current => {}
            other => {
                self.console.note(&other);
                if let Some(cause) = std::error::Error::source(&other) {
                    self.console.note(format!("caused by: {cause}"));
                }
            }
        }
        let probed = self.fleet.probe(&self.console);
        probed.up.iter().for_each(|snap| self.console.out(snap));
        for (host, error) in &probed.down {
            self.console.out(format!("{host}: down\n  {}", Chain(error).to_string().replace('\n', "\n  ")));
        }
        for snap in &probed.up {
            if let Some(member) = self.fleet.member(&snap.name) {
                match member.sweep() {
                    Ok(0) => {}
                    Ok(count) => {
                        self.console.note(format!("swept {count} job(s) older than seven days on `{}`", snap.name));
                    }
                    Err(error) => self.console.note(format!("sweeping `{}` failed: {}", snap.name, Chain(&error))),
                }
            }
        }
        Ok(if probed.up.is_empty() { Exit::Farm } else { Exit::Passed })
    }

    /// Prints a batch as text or as `uses.farm/v1`, and returns the exit it ends with.
    fn report(&self, batch: &Batch) -> Exit {
        let exit = batch
            .done
            .iter()
            .map(Finished::exit)
            .chain(batch.failed.iter().map(Error::exit))
            .fold(Exit::Passed, Exit::worst);
        if self.flags.json {
            let document = Document {
                schema: Document::SCHEMA,
                exit: exit.code(),
                jobs: batch.done.iter().map(Finished::wire).collect(),
                blessed: batch.blessed.clone(),
                formatted: batch.formatted.clone(),
            };
            match serde_json::to_string_pretty(&document) {
                Ok(text) => self.console.out(text),
                Err(error) => self.console.fail(format!("error: the report is not JSON: {error}")),
            }
        } else {
            batch.done.iter().for_each(|done| self.console.out(done));
        }
        batch.failed.iter().for_each(|error| self.console.fail(Chain(error)));
        if !self.flags.json && !batch.blessed.is_empty() {
            self.console.out("blessed (read each diff before committing):");
            batch.blessed.iter().for_each(|path| self.console.out(format!("  {path}")));
        }
        if !self.flags.json && !batch.formatted.is_empty() {
            self.console.out("formatted (`jj diff` shows what changed):");
            batch.formatted.iter().for_each(|path| self.console.out(format!("  {path}")));
        }
        if self.console.is_broken() { exit.worst(Exit::Farm) } else { exit }
    }
}

impl Need {
    /// The cargo flags that take their value as the next word.
    const VALUED: [&'static str; 18] = [
        "-p",
        "--package",
        "--exclude",
        "-F",
        "--features",
        "--bin",
        "--test",
        "--bench",
        "--example",
        "--manifest-path",
        "-E",
        "--target",
        "--target-dir",
        "--profile",
        "-j",
        "--jobs",
        "--color",
        "--message-format",
    ];

    /// A cargo command needs a golden host when it runs tests: its verb is `test`, `nextest`,
    /// `xtask` or `insta`. The verb is the first word that is not a flag or a flag's value.
    pub(crate) fn of_cargo(args: &[String]) -> Self {
        let mut words = args.iter().map(String::as_str);
        while let Some(word) = words.next() {
            if Self::VALUED.contains(&word) {
                words.next();
            } else if !word.starts_with('+') && !word.starts_with('-') {
                return match word {
                    "test" | "nextest" | "xtask" | "insta" => Self::Golden,
                    _ => Self::Any,
                };
            }
        }
        Self::Any
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::Member;
    use crate::fleet::fake::{Fake, Reply};
    use crate::host::{Host, Via};
    use crate::place::LeastLoaded;

    fn host(name: &str) -> Host {
        Host { name: name.parse().unwrap(), via: Via::Ssh("x".parse().unwrap()), ..Host::local() }
    }

    fn probe(cores: u32, arch: &str) -> Reply {
        Reply::ok(&[
            "now 1000",
            &format!("arch {arch}"),
            &format!("cores {cores}"),
            "load 0.10",
            "slots 1",
            "slot 0 free",
            "queued 0",
        ])
    }

    /// Each test's own workspace directory, so tests that write files do not meet.
    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn farm(members: Vec<Member<Fake>>) -> Farm<Fake, LeastLoaded> {
        let tree = Tree {
            root: std::env::temp_dir().join(format!(
                "farm-run-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            repo: "r".parse().unwrap(),
            workspace: "w".parse().unwrap(),
        };
        Farm::new(Setup {
            fleet: Fleet::new(members),
            placement: LeastLoaded,
            console: Arc::new(Console::default()),
            flags: Flags::default(),
            tree,
        })
    }

    #[test]
    fn a_check_fans_out_one_job_per_job_with_golden_jobs_on_golden_hosts() {
        let x86 = Member::new(host("x86"), Fake::saying(vec![probe(12, "x86_64")]));
        let arm = Member::new(host("arm"), Fake::saying(vec![probe(12, "aarch64")]));
        let farm = farm(vec![x86, arm]);
        let needs = [Need::Any, Need::Golden, Need::Any, Need::Any, Need::Golden];
        let chosen = farm.place(&needs).unwrap();
        let hosts: Vec<&str> = chosen.iter().map(|snap| snap.name.as_str()).collect();
        // lint takes the one free slot on `arm` or `x86`; both golden stages must be on x86
        assert_eq!(hosts.get(1), Some(&"x86"));
        assert_eq!(hosts.get(4), Some(&"x86"));
        assert!(hosts.contains(&"arm"), "{hosts:?}: free slots are used before anything queues");
    }

    #[test]
    fn a_golden_job_with_only_an_arm_host_is_an_error_naming_the_rule() {
        let arm = Member::new(host("arm"), Fake::saying(vec![probe(12, "aarch64")]));
        let error = farm(vec![arm]).place(&[Need::Golden]).unwrap_err();
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
        let member = Member::new(
            host("x86"),
            Fake::saying(vec![probe(12, "x86_64"), Reply::ok(&["@@farm:seed empty"]), events]),
        );
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
        assert!(batch.blessed.is_empty());
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
}
