//! `farm check`: every job of the repo, placed at once.
use crate::cache::{Kind as Tick, Moment, Trouble};
use std::num::NonZeroU32;
use std::thread;

use crate::cli::error::{Error, Listed};
use crate::cli::name::JobName;
use crate::cli::{Action, Usage};
use crate::config::jobs::{self, Jobs, Recipe};
use crate::host::Snapshot;
use crate::host::transport::Transport;
use crate::job::{Finished, Job, Totals, Verdict};
use crate::place::Placement;
use crate::run::{Batch, Farm};

/// What a stage that fails does to the stages running beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::run) enum Stop {
    /// Nothing: every job runs to its end.
    Never,
    /// The siblings still running or queued are cancelled.
    Siblings,
}

/// The stages that failed and the stages cancelled because of it, said once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::run) struct Stopped {
    pub(in crate::run) failed: Vec<String>,
    pub(in crate::run) stopped: Vec<String>,
}

impl Stopped {
    /// Sorts the results into a batch, and names the jobs that failed and the ones cancelled.
    fn collate(results: Vec<Result<Finished, Error>>, jobs: &[Job]) -> (Batch, Self) {
        let mut batch = Batch::default();
        let mut said = Self { failed: Vec::new(), stopped: Vec::new() };
        for (result, job) in results.into_iter().zip(jobs) {
            match result {
                Ok(done) => {
                    if done.verdict == Verdict::Cancelled {
                        said.stopped.push(done.label.clone());
                    } else if done.verdict.fails() {
                        said.failed.push(done.label.clone());
                    }
                    batch.done.push(done);
                }
                Err(Error::Cancelled { .. }) => said.stopped.push(job.label.clone()),
                Err(error) => {
                    said.failed.push(job.label.clone());
                    batch.failed.push(error);
                }
            }
        }
        (batch, said)
    }
}

impl std::fmt::Display for Stopped {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            out,
            "check: {} failed, so {} cancelled (`--keep-going` lets every stage finish)",
            Listed(&self.failed),
            Listed(&self.stopped)
        )
    }
}

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Runs the jobs at once, each on its placed host; the results come back in order.
    pub(in crate::run) fn fan(&self, jobs: &[Job], snaps: &[Snapshot]) -> Batch {
        self.spread(jobs, snaps, Stop::Never)
    }

    /// Like [`Self::fan`], and with [`Stop::Siblings`] the first job to fail cancels the ones
    /// still running or queued: they are recorded as cancelled, not failed, and never cached.
    pub(in crate::run) fn spread(&self, jobs: &[Job], snaps: &[Snapshot], stop: Stop) -> Batch {
        let attach = self.attach();
        let results: Vec<Result<Finished, Error>> = thread::scope(|scope| {
            let running: Vec<_> = jobs
                .iter()
                .zip(snaps)
                .map(|(job, snap)| {
                    scope.spawn(move || {
                        let result = self.run(job, snap, attach);
                        if stop == Stop::Siblings && Self::failed(&result) {
                            self.cancel.set();
                        }
                        result
                    })
                })
                .collect();
            running
                .into_iter()
                .zip(jobs)
                .map(|(handle, job)| {
                    handle.join().unwrap_or_else(|_| {
                        self.cancel.set();
                        Err(Error::Crashed { label: job.label.clone() })
                    })
                })
                .collect()
        });
        let (batch, said) = Stopped::collate(results, jobs);
        if !said.stopped.is_empty() {
            self.console.note(&said);
        }
        batch
    }

    /// Whether a job's result is a failure that cancels its siblings: a command that exited with
    /// a code, or a farm error. A cancelled job is neither.
    fn failed(result: &Result<Finished, Error>) -> bool {
        match result {
            Ok(done) => done.verdict.fails(),
            Err(Error::Cancelled { .. }) => false,
            Err(_) => true,
        }
    }

    /// The repo's declared jobs, one per job, all placed at once: the wall time is the slowest
    /// job, not the sum.
    pub(in crate::run) fn check(&self, names: &[JobName]) -> Result<Batch, Error> {
        let recipes = Jobs::load(&self.tree.root)?.pick(names)?;
        let jobs = if self.flags.affected {
            self.narrow(names, recipes)?
        } else {
            recipes.into_iter().map(Job::from).collect()
        };
        if jobs.is_empty() {
            return Ok(Batch::default());
        }
        let snaps = self.place(&jobs.iter().collect::<Vec<_>>())?;
        let stop = if names.is_empty() && !self.flags.keep_going { Stop::Siblings } else { Stop::Never };
        let batch = self.spread(&jobs, &snaps, stop);
        if names.is_empty() {
            self.full(&Totals::of(&batch.done));
        }
        Ok(batch)
    }

    /// The named jobs as nextest for the affected crates; each must declare `affected = true`.
    fn narrow(&self, names: &[JobName], recipes: Vec<Recipe>) -> Result<Vec<Job>, Error> {
        if names.is_empty() {
            return Err(Usage::Missing { verb: Action::Check, what: "the jobs to narrow with `--affected`" }.into());
        }
        if let Some(whole) = recipes.iter().find(|recipe| !recipe.narrow) {
            let path = Jobs::path(&self.tree.root);
            return Err(Error::Jobs { path, fault: jobs::Fault::Whole { name: whole.name.clone() } });
        }
        let choice = self.choose()?;
        Ok(recipes
            .into_iter()
            .filter_map(|recipe| {
                let job = choice.job(recipe.name.as_str(), &[])?;
                Some(Job { need: recipe.need, limit: recipe.limit, ..job })
            })
            .collect())
    }

    /// Counts a full `farm check` and warns when the day's count is over the budget: it never
    /// refuses, because a refusal would stall a real need.
    pub(in crate::run) fn full(&self, totals: &Totals) {
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
}
