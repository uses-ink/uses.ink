//! Running one job on a host.
mod serve;

use crate::cache::hash::Hash;
use crate::cache::{Claimed, Key, Kind as Tick, Trouble};
use crate::place::history::{Clock, Sample, Wall};
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::Seconds;
use crate::cli::error::Error;
use crate::cli::name::{HostName, JobId};
use crate::host::Snapshot;
use crate::host::Start;
use crate::host::transport::Transport;
use crate::job::event::{Event, Seeded};
use crate::job::summary::Summary;
use crate::job::{Attach, Finished, Flow, Job, LogFile, Served, Verdict};
use crate::place::Placement;
use crate::run::Farm;
/// Where a job's log is kept here, below the workspace: `target/farm/<host>.<token>.log`.
const LOGS: [&str; 2] = ["target", "farm"];

/// What a run knows of its place in the cache.
#[derive(Debug, Clone, Copy)]
pub(super) struct Plan {
    /// The key of a pass of this job.
    key: Key,
    /// The key of this job running in this workspace.
    run: Key,
    /// The digest of the inputs, as they were when the job started.
    digest: Hash,
}

/// A job and the host and id it is about to run as.
#[derive(Debug, Clone, Copy)]
pub(super) struct Turn<'a> {
    job: &'a Job,
    snap: &'a Snapshot,
    id: &'a JobId,
}

/// How a claim on a running job came out.
#[derive(Debug)]
pub(super) enum Held {
    /// This job runs, and the claim is released when it drops.
    Mine(Claimed),
    /// An identical job was running and this call waited for it.
    Other(Box<Finished>),
}

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    pub(in crate::run) fn next(&self, host: &HostName) -> JobId {
        let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis());
        JobId::new(host.clone(), millis, self.salt.fetch_add(1, Ordering::Relaxed))
    }

    pub(in crate::run) fn attach(&self) -> Attach {
        if self.flags.detach { Attach::Detach } else { Attach::Hold(self.flags.timeout.duration()) }
    }

    /// Says how a host's new tree came to be, when it did.
    pub(in crate::run) fn seeded(&self, host: &HostName, seeded: Option<Seeded>) {
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
    pub(in crate::run) fn announce(&self, job: &Job, snap: &Snapshot) {
        self.console.note(format!(
            "{}: on `{}`, {} of {} slots busy, {} queued",
            job.label, snap.name, snap.busy, snap.slots, snap.queued
        ));
    }

    /// The hard limit of a job: the person's `--limit`, else the job's own, else the default.
    pub(in crate::run) fn limit(&self, job: &Job) -> Seconds {
        self.flags.limit.or(job.limit).unwrap_or(Seconds::LIMIT)
    }

    /// Runs one job on the host `snap` describes: serve it from the passes, attach it to an
    /// identical running job, or sync, take a slot, run and summarise.
    pub(in crate::run) fn run(&self, job: &Job, snap: &Snapshot, attach: Attach) -> Result<Finished, Error> {
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

    /// Syncs, takes a slot, runs the job as `id` and summarises it.
    pub(in crate::run) fn launch(&self, turn: Turn<'_>, attach: Attach) -> Result<Finished, Error> {
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
    pub(in crate::run) fn learn(&self, job: &Job, done: Finished) -> Finished {
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

    pub(in crate::run) fn log_path(&self, id: &JobId) -> std::path::PathBuf {
        self.tree.root.join(LOGS[0]).join(LOGS[1]).join(format!("{id}.log"))
    }

    /// Copies a finished job's log from its host into this workspace; the path and the text.
    pub(in crate::run) fn fetch_log(
        &self,
        member: &crate::host::Member<T>,
        id: &JobId,
    ) -> Result<(std::path::PathBuf, String), Error> {
        let path = self.log_path(id);
        let text = member.log(id, None)?.output.join("\n");
        std::fs::create_dir_all(path.parent().unwrap_or(&self.tree.root))
            .and_then(|()| std::fs::write(&path, &text))
            .map_err(|source| Error::Io { path: path.clone(), source })?;
        Ok((path, text))
    }
}
