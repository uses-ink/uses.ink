//! Choosing hosts: the board, waking sleeping hosts, and placing jobs on them.
use crate::cache::Trouble;
use std::thread;

use crate::cli::error::{Chain, Error};
use crate::cli::name::HostName;
use crate::host::Probed;
use crate::host::transport::Transport;
use crate::host::{Arch, Snapshot};
use crate::job::Job;
use crate::place::{Ask, Placement, Why};
use crate::run::Farm;
impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// The hosts as placement sees them: those up, then the sleeping ones as they were last seen.
    /// A sleeping host never seen is left out unless `--host` names it, so it can be woken once.
    pub(in crate::run) fn board(&self, probed: &Probed) -> Vec<Snapshot> {
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
    pub(in crate::run) fn rouse(&self, chosen: &[Snapshot]) -> Vec<(HostName, Result<Snapshot, Error>)> {
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
    pub(in crate::run) fn waken(&self, jobs: &[&Job], chosen: &mut [Snapshot]) -> Result<Vec<HostName>, Error> {
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
    pub(in crate::run) fn place(&self, jobs: &[&Job]) -> Result<Vec<Snapshot>, Error> {
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
}
