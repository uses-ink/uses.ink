//! `farm bless` and `farm fmt`: run on a host, bring back only what changed.
use crate::cli::error::Error;
use crate::cli::{Action, Usage};
use crate::host::transport::Transport;
use crate::job::{Finished, Job, Reuse, Verdict};
use crate::place::history;
use crate::place::{Need, Placement};
use crate::run::changes::{Changes, Kind};
use crate::run::{Batch, Farm};
use std::time::SystemTime;

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Nextest with `INSTA_UPDATE=always` on a golden host, then only the changed `.snap` files
    /// come back. A snapshot edited here after the sync started is never overwritten, as in `fmt`.
    pub(in crate::run) fn bless(&self, args: &[String]) -> Result<Batch, Error> {
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
            ceiling: None,
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
    pub(in crate::run) fn fmt(&self) -> Result<Batch, Error> {
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
            ceiling: None,
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
    pub(in crate::run) fn fetch(
        &self,
        done: &Finished,
        kind: Kind,
        since: Option<SystemTime>,
    ) -> Result<Changes, Error> {
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
}
