//! `farm test` and `farm check <job> --affected`: nextest for the crates a change affects.
use crate::affected::{Affected, Choice, Diff, Fault, Layout};
use crate::cli::error::Error;
use crate::host::transport::Transport;
use crate::job::{Attach, Job, Reuse, Verdict};
use crate::place::history::Key;
use crate::place::{Need, Placement};
use crate::run::{Batch, Farm};

/// `cargo metadata` run on a placed host, in the synced workspace.
#[derive(Debug, Clone, Copy)]
pub(in crate::run) struct OnHost<'a, T, P>(pub(in crate::run) &'a Farm<T, P>);

impl<T: Transport + Sync, P: Placement + Sync> Layout for OnHost<'_, T, P> {
    fn json(&self) -> Result<String, Fault> {
        let farm = self.0;
        let words = ["cargo", "metadata", "--no-deps", "--format-version", "1"].map(str::to_owned).to_vec();
        let job = Job {
            label: "metadata".to_owned(),
            words,
            need: Need::Any,
            env: Vec::new(),
            limit: None,
            reuse: Reuse::Never,
            key: Key::stage("metadata"),
        };
        let host = |error| Fault::Host(Box::new(error));
        let snaps = farm.place(&[&job]).map_err(host)?;
        let snap = snaps.first().ok_or(Fault::NoPlace)?;
        let done = farm.run(&job, snap, Attach::Hold(farm.flags.timeout.duration())).map_err(host)?;
        match done.verdict {
            Verdict::Done { code: 0, .. } => {}
            Verdict::Done { code, .. } => return Err(Fault::Metadata { code, log: done.log }),
            Verdict::Running | Verdict::Aborted | Verdict::Lost => return Err(Fault::Running),
        }
        let text = std::fs::read_to_string(&done.log).map_err(|source| Fault::Log { log: done.log.clone(), source })?;
        text.lines().find(|line| line.starts_with('{')).map(str::to_owned).ok_or(Fault::NoJson { log: done.log })
    }
}

impl Choice {
    /// The nextest job for these crates, named `label`; none when nothing is affected. `need` and
    /// `limit` are the declared job's, when it is one.
    pub(in crate::run) fn job(&self, label: &str, args: &[String]) -> Option<Job> {
        let mut words: Vec<String> = ["nextest", "run"].map(str::to_owned).to_vec();
        words.extend(self.flags()?);
        words.extend(args.iter().cloned());
        Some(Job { label: label.to_owned(), ..Job::cargo(&words) })
    }
}

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Nextest for the affected crates, on the host placement picks.
    pub(in crate::run) fn test(&self, args: &[String]) -> Result<Batch, Error> {
        let Some(job) = self.choose()?.job("test", args) else {
            return Ok(Batch::default());
        };
        let snaps = self.place(&[&job])?;
        Ok(self.fan(&[job], &snaps))
    }

    /// The crates the change in this workspace affects, said in one line.
    pub(in crate::run) fn choose(&self) -> Result<Choice, Error> {
        let base = self.flags.base.clone().or_else(|| self.settings.scope.base.clone()).unwrap_or_default();
        let ask =
            Affected { layout: &OnHost(self), touch: &Diff { root: &self.tree.root }, scope: &self.settings.scope };
        let choice = ask.choose(&base)?;
        self.console.note(&choice);
        Ok(choice)
    }
}
