//! The verbs that use the hosts: [`Farm`] places jobs, runs them, and reports.

pub(crate) mod changes;
mod choose;
mod job;
pub(crate) mod overview;
mod report;
#[cfg(test)]
mod tests;
mod verb;

use crate::cache::{Cache, Moment, Trouble};
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use crate::cli::console::Console;
use crate::cli::error::Error;
use crate::cli::exit::Exit;
use crate::cli::{Flags, Usage, Verb};
use crate::config::jobs::Settings;
use crate::config::tree::Tree;
use crate::host::Fleet;
use crate::host::transport::Transport;
use crate::job::{Finished, Totals};
use crate::place::Placement;
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

    /// Probes the hosts and returns those that answered.
    #[cfg(test)]
    pub(crate) fn fleet_probe(&self) -> Vec<crate::host::Snapshot> {
        self.fleet.probe(&self.console).up
    }
}
