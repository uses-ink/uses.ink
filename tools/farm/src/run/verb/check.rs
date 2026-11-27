//! `farm check`: every job of the repo, placed at once.
use crate::cache::{Kind as Tick, Moment, Trouble};
use std::num::NonZeroU32;
use std::thread;

use crate::cli::error::Error;
use crate::cli::name::JobName;
use crate::config::jobs::Jobs;
use crate::host::Snapshot;
use crate::host::transport::Transport;
use crate::job::{Finished, Job, Totals};
use crate::place::Placement;
use crate::run::{Batch, Farm};

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Runs the jobs at once, each on its placed host; the results come back in order.
    pub(in crate::run) fn fan(&self, jobs: &[Job], snaps: &[Snapshot]) -> Batch {
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
    pub(in crate::run) fn check(&self, names: &[JobName]) -> Result<Batch, Error> {
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
