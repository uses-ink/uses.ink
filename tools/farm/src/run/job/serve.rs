//! A job and the cache: served from a pass, attached to a running twin, or kept.
use crate::cache::inputs::Toolchain;
use crate::cache::{Causes, Claim, Entry, Facts, Flaky, Key, Kind as Tick, Mark, Marks, Moment, Record, Trouble};
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::Seconds;
use crate::cli::error::Error;
use crate::cli::name::JobId;
use crate::host::Snapshot;
use crate::host::transport::Transport;
use crate::job::event::Event;
use crate::job::summary::Summary;
use crate::job::{Finished, Job, Served, Verdict};
use crate::place::{Need, Placement};
use crate::run::Farm;
use crate::run::job::{Held, Plan, Turn};
impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Works out what a pass of the job would be keyed by, when it may be reused; a digest that
    /// cannot be made says why and leaves the job to run uncached.
    pub(in crate::run) fn plan(&self, job: &Job, snap: &Snapshot) -> Option<Plan> {
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

    pub(in crate::run) fn skipped(&self, label: &str, trouble: &Trouble) {
        self.console.note(format!("{label}: not cached, {}", Causes(trouble)));
    }

    /// The pass this job already has, as a result, when one was remembered.
    pub(in crate::run) fn reuse(&self, job: &Job, plan: &Plan) -> Option<Finished> {
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
    pub(in crate::run) fn claim(&self, plan: &Plan, turn: Turn<'_>) -> Result<Held, Trouble> {
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
    pub(in crate::run) fn join(&self, job: &Job, mark: &Mark) -> Result<Option<Finished>, Error> {
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
    pub(in crate::run) fn joined(job: &Job, id: &JobId, verdict: Verdict, log: std::path::PathBuf) -> Finished {
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
    pub(in crate::run) fn keep(&self, job: &Job, plan: &Plan, done: &Finished) {
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
    pub(in crate::run) fn promote(&self, id: &JobId, ended: (i32, Duration), logged: (&std::path::Path, &str)) {
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
    pub(in crate::run) fn record(&self, kind: Tick, job: &str, spent: (u64, u32)) {
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
}
