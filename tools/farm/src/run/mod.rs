//! The verbs that use the hosts: [`Farm`] places jobs, runs them, and reports.

pub(crate) mod changes;
pub(crate) mod overview;

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::cache::hash::Hash;
use crate::cache::inputs::Toolchain;
use crate::cache::{
    Cache, Causes, Claim, Claimed, Entry, Facts, Flaky, Key, Kind as Tick, Mark, Marks, Moment, Record, Trouble,
};
use crate::place::history::{self, Clock, Sample, Wall};

use crate::cli::console::Console;
use crate::cli::error::{Chain, Error};
use crate::cli::exit::Exit;
use crate::cli::name::{HostName, JobId, JobName};
use crate::cli::version::{Freshness, Version};
use crate::cli::{Action, Flags, Seconds, Usage, Verb};
use crate::config::jobs::{Jobs, Settings};
use crate::config::tree::Tree;
use crate::host::transport::Transport;
use crate::host::{Arch, Snapshot};
use crate::host::{Fleet, Probed, Start};
use crate::job::event::{Event, Seeded};
use crate::job::summary::Summary;
use crate::job::{Attach, Document, Finished, Flow, Job, LogFile, Reuse, Served, Totals, Verdict};
use crate::place::{Ask, Need, Placement, Why};
use crate::run::changes::{Changes, Kind};
use crate::run::overview::Overview;

/// Where a job's log is kept here, below the workspace: `target/farm/<host>.<token>.log`.
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
    pub(crate) settings: Settings,
    /// What the farm remembers on this machine; none turns reuse and dedupe off.
    pub(crate) cache: Option<Cache>,
}

/// What a run knows of its place in the cache.
#[derive(Debug, Clone, Copy)]
struct Plan {
    /// The key of a pass of this job.
    key: Key,
    /// The key of this job running in this workspace.
    run: Key,
    /// The digest of the inputs, as they were when the job started.
    digest: Hash,
}

/// A job and the host and id it is about to run as.
#[derive(Debug, Clone, Copy)]
struct Turn<'a> {
    job: &'a Job,
    snap: &'a Snapshot,
    id: &'a JobId,
}

/// How a claim on a running job came out.
#[derive(Debug)]
enum Held {
    /// This job runs, and the claim is released when it drops.
    Mine(Claimed),
    /// An identical job was running and this call waited for it.
    Other(Box<Finished>),
}

/// The farm: the hosts, the placement, the workspace, and what the person asked for.
#[derive(Debug)]
pub(crate) struct Farm<T, P> {
    fleet: Fleet<T>,
    placement: P,
    console: Arc<Console>,
    flags: Flags,
    tree: Tree,
    settings: Settings,
    cache: Option<Cache>,
    salt: AtomicU32,
}

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    pub(crate) fn new(setup: Setup<T, P>) -> Self {
        let Setup { fleet, placement, console, flags, tree, settings, cache } = setup;
        Self { fleet, placement, console, flags, tree, settings, cache, salt: AtomicU32::new(0) }
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
        let exit = self.report(&batch);
        if matches!(verb, Verb::Check(_)) {
            self.console.note(Totals::of(&batch.done));
        }
        Ok(exit)
    }

    /// The hosts as placement sees them: those up, then the sleeping ones as they were last seen.
    /// A sleeping host never seen is left out unless `--host` names it, so it can be woken once.
    fn board(&self, probed: &Probed) -> Vec<Snapshot> {
        let mut board = probed.up.clone();
        if let Some(cache) = &self.cache {
            for snap in
                probed.up.iter().filter(|snap| self.fleet.member(&snap.name).is_some_and(|m| m.host.wake.is_some()))
            {
                if let Err(fault) = cache.seen.note(snap) {
                    self.skipped(snap.name.as_str(), &Trouble::Store(fault));
                }
            }
        }
        for host in &probed.asleep {
            let Some(wake) = &host.wake else { continue };
            let seen = self.cache.as_ref().and_then(|cache| {
                cache
                    .seen
                    .recall(host, wake.cost)
                    .map_err(|fault| self.skipped(host.name.as_str(), &Trouble::Store(fault)))
                    .ok()?
            });
            let pinned = self.flags.host.as_ref() == Some(&host.name);
            board.extend(seen.or_else(|| pinned.then(|| Snapshot::asleep(host, Arch::REFERENCE, (1, 1), wake.cost))));
        }
        board
    }

    /// Wakes the sleeping hosts among `chosen`, each once and all at once, and says so.
    fn rouse(&self, chosen: &[Snapshot]) -> Vec<(HostName, Result<Snapshot, Error>)> {
        let mut names: Vec<HostName> = Vec::new();
        for snap in chosen.iter().filter(|snap| snap.asleep.is_some()) {
            if !names.contains(&snap.name) {
                self.console.note(format!(
                    "`{}` is asleep: waking it (about {} s)",
                    snap.name,
                    snap.asleep.unwrap_or(0)
                ));
                names.push(snap.name.clone());
            }
        }
        thread::scope(|scope| {
            let running: Vec<_> = names
                .iter()
                .map(|name| scope.spawn(move || self.fleet.member(name).map(crate::host::Member::wake)))
                .collect();
            names
                .iter()
                .cloned()
                .zip(running)
                .map(|(name, handle)| {
                    let result =
                        handle.join().ok().flatten().unwrap_or_else(|| Err(Error::Crashed { label: name.to_string() }));
                    (name, result)
                })
                .collect()
        })
    }

    /// Wakes the sleeping hosts the jobs were placed on. A host that cannot be woken is said,
    /// and returned so placement can leave it out; a pinned one is an error.
    fn waken(&self, jobs: &[&Job], chosen: &mut [Snapshot]) -> Result<Vec<HostName>, Error> {
        let woken = self.rouse(chosen);
        let mut failed = Vec::new();
        for (name, result) in woken {
            match result {
                Ok(real) => {
                    for (snap, job) in chosen.iter_mut().zip(jobs).filter(|(snap, _)| snap.name == name) {
                        if !job.need.fits(&real) {
                            return Err(Error::NotGolden { host: name });
                        }
                        *snap = real.clone();
                    }
                    if let Some(Err(fault)) = self.cache.as_ref().map(|cache| cache.seen.note(&real)) {
                        self.skipped(name.as_str(), &Trouble::Store(fault));
                    }
                }
                Err(error) if self.flags.host.as_ref() == Some(&name) => return Err(error),
                Err(error) => {
                    let cause =
                        std::error::Error::source(&error).map_or_else(String::new, |source| format!(": {source}"));
                    self.console.note(format!("{error}{cause}; leaving `{name}` out of this placement"));
                    failed.push(name);
                }
            }
        }
        Ok(failed)
    }

    /// Places `jobs` on hosts at once: each is reserved a slot or a place in the queue before
    /// the next is placed, so a fan-out spreads over the hosts. A sleeping host that is chosen
    /// is woken; one that cannot be is left out and the jobs are placed again.
    fn place(&self, jobs: &[&Job]) -> Result<Vec<Snapshot>, Error> {
        let probed = self.fleet.probe(&self.console);
        for (host, error) in &probed.down {
            self.console.note(format!("`{host}` is down and left out: {}", Chain(error)));
        }
        let mut left_out: Vec<HostName> = Vec::new();
        loop {
            let mut board: Vec<Snapshot> =
                self.board(&probed).into_iter().filter(|snap| !left_out.contains(&snap.name)).collect();
            let mut chosen = Vec::new();
            let mut said = Vec::new();
            for job in jobs {
                let ask = Ask { need: job.need, key: &job.key, pin: self.flags.host.as_ref() };
                let choice = self.placement.choose(ask, &board)?;
                if choice.why != Why::Only {
                    said.push(format!("{}: {choice}", job.label));
                }
                if let Some(snap) = board.get_mut(choice.at) {
                    chosen.push(snap.clone());
                    snap.reserve();
                }
            }
            let failed = self.waken(jobs, &mut chosen)?;
            if failed.is_empty() {
                for line in &said {
                    self.console.note(line);
                }
                return Ok(chosen);
            }
            left_out.extend(failed);
        }
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

    /// Runs one job on the host `snap` describes: serve it from the passes, attach it to an
    /// identical running job, or sync, take a slot, run and summarise.
    fn run(&self, job: &Job, snap: &Snapshot, attach: Attach) -> Result<Finished, Error> {
        let id = self.next(&snap.name);
        let turn = Turn { job, snap, id: &id };
        let Some(plan) = self.plan(job, snap) else {
            return self.launch(turn, attach);
        };
        if !self.flags.fresh
            && let Some(done) = self.reuse(job, &plan)
        {
            return Ok(done);
        }
        let mut claimed = match self.claim(&plan, turn) {
            Ok(Held::Mine(claimed)) => Some(claimed),
            Ok(Held::Other(done)) => return Ok(*done),
            Err(trouble) => {
                self.skipped(&job.label, &trouble);
                None
            }
        };
        let done = self.launch(turn, attach)?;
        self.record(Tick::Stage, &job.label, (done.ran(), 0));
        match done.verdict {
            Verdict::Running => {
                if let Some(claimed) = claimed.as_mut() {
                    claimed.stay();
                }
            }
            Verdict::Done { code: 0, .. } => self.keep(job, &plan, &done),
            Verdict::Done { .. } | Verdict::Aborted | Verdict::Lost => {}
        }
        Ok(done)
    }

    /// Works out what a pass of the job would be keyed by, when it may be reused; a digest that
    /// cannot be made says why and leaves the job to run uncached.
    fn plan(&self, job: &Job, snap: &Snapshot) -> Option<Plan> {
        let cache = self.cache.as_ref().filter(|_| job.reuse.allowed())?;
        let made = (|| -> Result<Plan, Trouble> {
            let digest = cache.inputs.digest(&self.tree.root, job.reuse.files(), false)?;
            let channel = Toolchain::channel(&self.tree.root)?;
            let arch = (job.need == Need::Golden).then(|| snap.arch.to_string());
            let key = Key::of(&Facts {
                job: &job.label,
                words: &job.words,
                env: &job.env,
                digest,
                arch: arch.as_deref(),
                channel: channel.as_deref(),
                config: self.settings.config,
            });
            Ok(Plan { key, run: key.within(self.tree.workspace.as_str()), digest })
        })();
        made.map_err(|trouble| self.skipped(&job.label, &trouble)).ok()
    }

    fn skipped(&self, label: &str, trouble: &Trouble) {
        self.console.note(format!("{label}: not cached, {}", Causes(trouble)));
    }

    /// The pass this job already has, as a result, when one was remembered.
    fn reuse(&self, job: &Job, plan: &Plan) -> Option<Finished> {
        let cache = self.cache.as_ref()?;
        let hit = match cache.store.lookup(plan.key, Moment::now()) {
            Ok(hit) => hit?,
            Err(fault) => {
                self.skipped(&job.label, &Trouble::Store(fault));
                return None;
            }
        };
        let id: JobId = hit.entry.id.parse().ok()?;
        let log = std::fs::read_to_string(&hit.entry.log).unwrap_or_default();
        self.record(Tick::Hit, &job.label, (0, 0));
        Some(Finished {
            summary: Summary::of(&log),
            host: id.host.clone(),
            id,
            label: job.label.clone(),
            command: job.shown(),
            slot: None,
            queued: Duration::ZERO,
            verdict: Verdict::Done { code: 0, ran: Duration::from_secs(hit.entry.ran) },
            log: hit.entry.log,
            served: Served::Cached { ago: hit.ago },
        })
    }

    /// Marks the job as running, or attaches to the identical one that is.
    fn claim(&self, plan: &Plan, turn: Turn<'_>) -> Result<Held, Trouble> {
        let Turn { job, snap, id } = turn;
        let Some(cache) = &self.cache else {
            return Err(Trouble::Off);
        };
        let mark = Mark {
            id: id.to_string(),
            pid: std::process::id(),
            key: plan.key.hash(),
            job: job.label.clone(),
            command: job.shown(),
            workspace: self.tree.workspace.to_string(),
            host: snap.name.to_string(),
            digest: plan.digest,
        };
        for _ in 0..2 {
            match cache.marks.claim(plan.run, &mark)? {
                Claim::Won(claimed) => return Ok(Held::Mine(claimed)),
                Claim::Held(running) => {
                    if let Some(done) = self.join(job, &running).map_err(Trouble::Attach)? {
                        self.record(Tick::Attach, &job.label, (0, 0));
                        return Ok(Held::Other(Box::new(done)));
                    }
                    cache.marks.release(plan.run);
                }
            }
        }
        Err(Trouble::Store(crate::cache::Fault::Io {
            path: std::path::PathBuf::from("running"),
            source: std::io::Error::other("an identical job keeps appearing and vanishing"),
        }))
    }

    /// Waits for the identical job `mark` names, as `farm wait` would; none when it is gone.
    fn join(&self, job: &Job, mark: &Mark) -> Result<Option<Finished>, Error> {
        let Ok(id) = mark.id.parse::<JobId>() else {
            return Ok(None);
        };
        let Some(member) = self.fleet.member(&id.host) else {
            return Ok(None);
        };
        self.console.note(format!(
            "{}: an identical job is already running, {id}; attaching to it, not starting another",
            job.label
        ));
        let started = Instant::now();
        loop {
            let left = self.flags.timeout.duration().saturating_sub(started.elapsed());
            let wait = if self.flags.detach { Seconds(0) } else { Seconds(left.as_secs()) };
            let said = member.wait(&id, wait)?;
            match said.events.first() {
                Some(Event::Running) => return Ok(Some(Self::joined(job, &id, Verdict::Running, self.log_path(&id)))),
                Some(Event::Done { code, ran }) => {
                    let (path, text) = self.fetch_log(member, &id)?;
                    let mut done = Self::joined(job, &id, Verdict::Done { code: *code, ran: *ran }, path);
                    done.summary = Summary::of(&text);
                    self.promote(&id, (*code, *ran), (&done.log, &text));
                    return Ok(Some(done));
                }
                Some(Event::NoJob) if Marks::alive(mark.pid) && started.elapsed() < self.flags.timeout.duration() => {
                    thread::sleep(Duration::from_secs(1));
                }
                _ => return Ok(None),
            }
        }
    }

    /// The result of a call that waited for the identical job `id`.
    fn joined(job: &Job, id: &JobId, verdict: Verdict, log: std::path::PathBuf) -> Finished {
        Finished {
            id: id.clone(),
            label: job.label.clone(),
            command: job.shown(),
            host: id.host.clone(),
            slot: None,
            queued: Duration::ZERO,
            verdict,
            log,
            summary: Summary::default(),
            served: Served::Attached,
        }
    }

    /// Remembers the pass of a foreground job that finished, when it was clean and the tree it
    /// read is still the tree it was keyed by.
    fn keep(&self, job: &Job, plan: &Plan, done: &Finished) {
        let Some(cache) = &self.cache else {
            return;
        };
        let text = std::fs::read_to_string(&done.log).unwrap_or_default();
        if Flaky::shows(&text) {
            self.console.note(format!(
                "{}: passed, but its log shows a retry or a timeout, so the pass is not kept",
                job.label
            ));
            return;
        }
        let same = cache.inputs.digest(&self.tree.root, job.reuse.files(), true).is_ok_and(|now| now == plan.digest);
        if !same {
            self.console
                .note(format!("{}: passed, but the tree changed while it ran, so the pass is not kept", job.label));
            return;
        }
        let entry = Entry {
            job: job.label.clone(),
            command: job.shown(),
            id: done.id.to_string(),
            host: done.host.to_string(),
            workspace: self.tree.workspace.to_string(),
            at: Moment::now(),
            ran: done.ran(),
            log: done.log.clone(),
            digest: plan.digest,
        };
        if let Err(fault) = cache.store.record(plan.key, &entry, Moment::now()) {
            self.skipped(&job.label, &Trouble::Store(fault));
        }
    }

    /// A detached job that ended under `farm wait` or an attach: its mark goes, and its pass is
    /// kept when it was clean. The tree is not looked at again: it may have been edited since,
    /// and the digest in the mark is the one the job started with.
    fn promote(&self, id: &JobId, ended: (i32, Duration), logged: (&std::path::Path, &str)) {
        let (code, ran) = ended;
        let (log, text) = logged;
        let Some(cache) = &self.cache else {
            return;
        };
        let held = match cache.marks.holding(&id.to_string()) {
            Ok(held) => held,
            Err(fault) => return self.skipped(&id.to_string(), &Trouble::Store(fault)),
        };
        let Some((run, mark)) = held else {
            return;
        };
        cache.marks.release(run);
        if code != 0 || Flaky::shows(text) {
            return;
        }
        let entry = Entry {
            job: mark.job.clone(),
            command: mark.command,
            id: mark.id,
            host: mark.host,
            workspace: mark.workspace,
            at: Moment::now(),
            ran: ran.as_secs(),
            log: log.to_path_buf(),
            digest: mark.digest,
        };
        if let Err(fault) = cache.store.record(Marks::key(mark.key), &entry, Moment::now()) {
            self.skipped(&mark.job, &Trouble::Store(fault));
        }
    }

    /// Adds a line to the journal; a journal that cannot be written is said and the run goes on.
    fn record(&self, kind: Tick, job: &str, spent: (u64, u32)) {
        let (secs, count) = spent;
        let Some(cache) = &self.cache else {
            return;
        };
        let record = Record {
            at: Moment::now(),
            workspace: self.tree.workspace.to_string(),
            kind,
            job: job.to_owned(),
            secs,
            count,
        };
        if let Err(fault) = cache.journal.append(&record) {
            self.skipped(job, &Trouble::Store(fault));
        }
    }

    fn log_path(&self, id: &JobId) -> std::path::PathBuf {
        self.tree.root.join(LOGS[0]).join(LOGS[1]).join(format!("{id}.log"))
    }

    /// Copies a finished job's log from its host into this workspace; the path and the text.
    fn fetch_log(&self, member: &crate::host::Member<T>, id: &JobId) -> Result<(std::path::PathBuf, String), Error> {
        let path = self.log_path(id);
        let text = member.log(id, None)?.output.join("\n");
        std::fs::create_dir_all(path.parent().unwrap_or(&self.tree.root))
            .and_then(|()| std::fs::write(&path, &text))
            .map_err(|source| Error::Io { path: path.clone(), source })?;
        Ok((path, text))
    }

    /// Syncs, takes a slot, runs the job as `id` and summarises it.
    fn launch(&self, turn: Turn<'_>, attach: Attach) -> Result<Finished, Error> {
        let Turn { job, snap, id } = turn;
        let member = self
            .fleet
            .member(&snap.name)
            .ok_or_else(|| Error::NoHost { need: job.need, up: vec![snap.name.clone()] })?;
        self.announce(job, snap);
        self.seeded(&snap.name, member.sync(&self.tree, &self.settings.exclude)?);
        let log = self.log_path(id);
        let file = LogFile::create(&log).map_err(|source| Error::Io { path: log.clone(), source })?;
        let mut flow = Flow { file: Some(file), ..Flow::default() };
        let call = member.launch(&Start { tree: &self.tree, snap, job, id }, attach, self.limit(job));
        let eta = snap.eta().map_or_else(|| "unknown".to_owned(), |eta| format!("about {} s", eta.as_secs()));
        member.run(&call, &mut |text| {
            if let Some(Event::Queued { behind }) = flow.feed(text) {
                self.console.note(format!("{}: queued behind {behind}, host `{}` free in {eta}", job.label, snap.name));
            }
        })?;
        let verdict = flow.settle(&snap.name, id, &log)?;
        Ok(self.learn(
            job,
            Finished {
                summary: Summary::of(&flow.log),
                id: id.clone(),
                label: job.label.clone(),
                command: job.shown(),
                host: snap.name.clone(),
                slot: flow.slot,
                queued: flow.queued,
                verdict,
                log,
                served: Served::Ran,
            },
        ))
    }

    /// Files how long a finished job took on its host, for placement; the job's result as it was.
    fn learn(&self, job: &Job, done: Finished) -> Finished {
        let (Some(cache), Verdict::Done { code, ran }) = (&self.cache, done.verdict) else {
            return done;
        };
        let sample = Sample {
            at: Wall.now(),
            key: job.key.clone(),
            host: done.host.to_string(),
            secs: ran.as_secs(),
            passed: code == 0,
        };
        if let Err(fault) = cache.history.append(&sample) {
            self.skipped(&job.label, &Trouble::Store(fault));
        }
        done
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
        let snaps = self.place(&jobs.iter().collect::<Vec<_>>())?;
        let batch = self.fan(&jobs, &snaps);
        if names.is_empty() {
            self.full(&Totals::of(&batch.done));
        }
        Ok(batch)
    }

    /// Counts a full `farm check` and warns when the day's count is over the budget: it never
    /// refuses, because a refusal would stall a real need.
    fn full(&self, totals: &Totals) {
        let ran = totals.ran;
        self.record(Tick::Check, "check", (totals.secs, ran));
        let Some(budget) = self.flags.budget.or_else(|| self.settings.budget.map(NonZeroU32::get)) else {
            return;
        };
        let Some(cache) = &self.cache else {
            return;
        };
        let tallies = match cache.journal.tally(Moment::now()) {
            Ok(tallies) => tallies,
            Err(fault) => return self.skipped("budget", &Trouble::Store(fault)),
        };
        let own = self.tree.workspace.as_str();
        let heavy = tallies.iter().find(|tally| tally.workspace == own).map_or(0, |tally| tally.heavy);
        if ran > 0 && heavy > budget {
            self.console.note(format!(
                "warning: full check {heavy} in the last 24 hours in workspace `{own}`, over the budget of {budget}; run one full check per task and rerun only the stage that failed"
            ));
        }
    }

    /// Any cargo command, on a placed host.
    fn cargo(&self, args: &[String]) -> Result<Batch, Error> {
        let job = Job::cargo(args);
        let snaps = self.place(&[&job])?;
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
        let job = Job {
            label: "bless".to_owned(),
            words,
            need: Need::Golden,
            env,
            limit: None,
            reuse: Reuse::Never,
            key: history::Key::stage("bless"),
        };
        let snaps = self.place(&[&job])?;
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
        let job = Job {
            label: "fmt".to_owned(),
            words,
            need: Need::Any,
            env: Vec::new(),
            limit: None,
            reuse: Reuse::Never,
            key: history::Key::stage("fmt"),
        };
        let snaps = self.place(&[&job])?;
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
            served: Served::Ran,
        };
        let path = self.log_path(id);
        match said.events.first() {
            Some(Event::Done { code, ran }) => {
                let (path, text) = self.fetch_log(member, id)?;
                self.promote(id, (*code, *ran), (&path, &text));
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

    /// Every host: arch, load, slots, queue, jobs, the timings placement uses; and the old job
    /// state swept.
    fn status(&self) -> Result<Exit, Error> {
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
        let jobs: Vec<Job> = Jobs::load_optional(&self.tree.root)?
            .map(|jobs| jobs.pick(&[]))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(Job::from)
            .collect();
        let board = self.board(&probed);
        let overview = Overview::of(&probed, &board, &self.placement, &jobs);
        if self.flags.json {
            match serde_json::to_string_pretty(&overview) {
                Ok(text) => self.console.out(text),
                Err(error) => self.console.fail(format!("error: the status is not JSON: {error}")),
            }
        } else {
            probed.up.iter().for_each(|snap| self.console.out(snap));
            overview.hosts.iter().for_each(|row| self.console.out(row));
            overview.asleep.iter().for_each(|sleeping| self.console.out(sleeping));
            overview.next.iter().for_each(|next| self.console.out(next));
            self.usage();
            for (host, error) in &probed.down {
                self.console.out(format!("{host}: down\n  {}", Chain(error).to_string().replace('\n', "\n  ")));
            }
        }
        self.sweep(&probed);
        Ok(if probed.up.is_empty() { Exit::Farm } else { Exit::Passed })
    }

    /// Removes the state of old jobs on every host that is up, and says how many.
    fn sweep(&self, probed: &Probed) {
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
    }

    /// What each workspace of this repo ran in the last 24 hours, so heavy use is visible.
    fn usage(&self) {
        let Some(cache) = &self.cache else {
            return;
        };
        match cache.journal.tally(Moment::now()) {
            Ok(tallies) if tallies.is_empty() => self.console.out("last 24 h: no runs recorded"),
            Ok(tallies) => {
                for tally in tallies {
                    let here = if tally.workspace == self.tree.workspace.as_str() { " (this one)" } else { "" };
                    self.console.out(format!("last 24 h, {tally}{here}"));
                }
            }
            Err(fault) => self.skipped("status", &Trouble::Store(fault)),
        }
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
    const VALUED: [&'static str; 21] = [
        "-C",
        "--config",
        "-Z",
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

    /// The cargo subcommand: the first word that is not a flag or a flag's value.
    pub(crate) fn subcommand(args: &[String]) -> Option<&str> {
        let mut words = args.iter().map(String::as_str);
        while let Some(word) = words.next() {
            if Self::VALUED.contains(&word) {
                words.next();
            } else if !word.starts_with('+') && !word.starts_with('-') {
                return Some(word);
            }
        }
        None
    }

    /// A cargo command needs a golden host when it runs tests: its verb is `test`, `nextest`,
    /// `xtask` or `insta`.
    pub(crate) fn of_cargo(args: &[String]) -> Self {
        match Self::subcommand(args) {
            Some("test" | "nextest" | "xtask" | "insta") => Self::Golden,
            _ => Self::Any,
        }
    }
}

impl Reuse {
    /// A cargo command that only answers a question (check, lint, test) may be served from an
    /// earlier pass; one that leaves something behind for the caller (build, run, install) never.
    pub(crate) fn of_cargo(args: &[String]) -> Self {
        match Need::subcommand(args) {
            Some("check" | "clippy" | "test" | "doc" | "deny") => Self::Whole,
            Some("nextest") if args.iter().any(|arg| arg == "run") => Self::Whole,
            _ => Self::Never,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Member;
    use crate::host::fake::{Fake, Reply};
    use crate::host::{Host, Via};
    use crate::place::LeastLoaded;
    use crate::place::wake::Wake;

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
        Farm::<Fake, LeastLoaded>::with(members, Flags::default(), None)
    }

    impl Farm<Fake, LeastLoaded> {
        /// A farm over fakes, with these flags and, when given, a cache kept in this directory.
        fn with(members: Vec<Member<Fake>>, flags: Flags, cache: Option<&std::path::Path>) -> Self {
            let settings = Settings::default();
            let cache = cache.map(|dir| {
                let inputs =
                    crate::cache::inputs::Inputs::new(Arc::new(crate::cache::inputs::Walk), &settings.exclude).unwrap();
                Cache::at(dir, settings.limits, settings.memory, inputs)
            });
            Self::built(members, flags, cache)
        }

        fn built(members: Vec<Member<Fake>>, flags: Flags, cache: Option<Cache>) -> Self {
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
                flags,
                tree,
                settings: Settings::default(),
                cache,
            })
        }
    }

    /// A scratch directory and a script in it that records its arguments in `marker` and exits
    /// with `code`.
    struct Wakeup {
        dir: std::path::PathBuf,
    }

    impl Wakeup {
        fn new(code: i32) -> Self {
            use std::os::unix::fs::PermissionsExt as _;
            let dir = std::env::temp_dir().join(format!(
                "farm-wake-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let script = dir.join("wake.sh");
            let body = format!("#!/bin/sh\necho \"$@\" >> {}\nexit {code}\n", dir.join("marker").display());
            std::fs::write(&script, body).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            Self { dir }
        }

        fn host(&self, name: &str) -> Host {
            let command = self.dir.join("wake.sh").display().to_string();
            Host { wake: Wake::new(&command, Some(Seconds(5)), Some(Seconds(120))), ..host(name) }
        }

        /// How many times the script ran, and with what.
        fn woken(&self) -> Vec<String> {
            std::fs::read_to_string(self.dir.join("marker"))
                .map_or_else(|_| Vec::new(), |text| text.lines().map(str::to_owned).collect())
        }
    }

    impl Drop for Wakeup {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn silent() -> Reply {
        Reply { lines: vec![], code: 255 }
    }

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

    fn asking(need: Need) -> Job {
        Job {
            label: "x".to_owned(),
            words: vec!["true".to_owned()],
            need,
            env: Vec::new(),
            limit: None,
            reuse: Reuse::Never,
            key: history::Key::stage("x"),
        }
    }

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
}
