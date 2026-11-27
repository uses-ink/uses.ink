//! Where a job goes: which hosts may run it, and which of them is the best one now.

mod fastest;
pub(crate) mod history;
pub(crate) mod least;
#[cfg(test)]
mod tests;
pub(crate) mod wake;

use std::fmt;

pub(crate) use fastest::Fastest;

use crate::cli::error::Error;
use crate::cli::name::HostName;
use crate::host::{Load, Snapshot};
use crate::place::history::{Key, Median};

/// What a host must be for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Need {
    /// Any host.
    Any,
    /// A golden host: output-affecting tests and goldens run only on the reference architecture,
    /// or on one the config declares golden.
    Golden,
}

impl Need {
    /// Whether the host `snap` describes may run the job.
    pub(crate) const fn fits(self, snap: &Snapshot) -> bool {
        match self {
            Self::Any => true,
            Self::Golden => snap.golden,
        }
    }
}

impl fmt::Display for Need {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Any => "any",
            Self::Golden => "golden",
        })
    }
}

/// What a job asks of placement.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ask<'a> {
    pub(crate) need: Need,
    /// What the job is, for its history.
    pub(crate) key: &'a Key,
    /// The only host allowed, when the user chose one.
    pub(crate) pin: Option<&'a HostName>,
    /// The most load a host may carry, when the job needs a quiet one: a busier host does not
    /// run it, and with every host busier it waits.
    pub(crate) ceiling: Option<Load>,
}

/// How an estimate came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// The host's own median for the key.
    Median,
    /// The other hosts' medians for the key, scaled by core count.
    Scaled,
}

/// When a host is expected to finish a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Estimate {
    pub(crate) host: HostName,
    /// Seconds from now to the end of the job, wait for a slot included.
    pub(crate) secs: u64,
    /// Jobs that must finish before a slot is free for it.
    pub(crate) queue: u32,
    pub(crate) basis: Basis,
    /// Seconds of `secs` that are the host starting up; zero for a host that is up.
    pub(crate) wake: u64,
    /// Seconds of `secs` that are the host's configured handicap.
    pub(crate) handicap: u64,
}

impl fmt::Display for Estimate {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scaled = if self.basis == Basis::Scaled { ", scaled from other hosts" } else { "" };
        let wake = if self.wake > 0 { format!(", of which {} s to wake it", self.wake) } else { String::new() };
        let handicap =
            if self.handicap > 0 { format!(", of which {} s handicap", self.handicap) } else { String::new() };
        write!(out, "{} (est {} s, queue {}{scaled}{wake}{handicap})", self.host, self.secs, self.queue)
    }
}

/// Why a host was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Why {
    /// `--host` named it.
    Pinned,
    /// No other host may run the job.
    Only,
    /// No host has a time for this job yet: the least loaded.
    Load,
    /// The host has no history at all and a slot is free: it is tried once.
    Trying,
    /// The lowest expected finish, over the runner-up.
    Estimated { best: Estimate, over: Option<Estimate> },
}

/// The host chosen, as an index into the board, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) at: usize,
    pub(crate) host: HostName,
    pub(crate) why: Why,
}

impl fmt::Display for Choice {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host = &self.host;
        match &self.why {
            Why::Pinned => write!(out, "{host} (pinned with --host)"),
            Why::Only => write!(out, "{host} (the only host that may run it)"),
            Why::Load => write!(out, "{host} (no timings for it yet; the least loaded)"),
            Why::Trying => write!(out, "{host} (no history on this host yet; trying it once)"),
            Why::Estimated { best, over: None } => write!(out, "{best}"),
            Why::Estimated { best, over: Some(over) } => {
                let queue = if over.queue > 0 { format!(", queue {}", over.queue) } else { String::new() };
                write!(out, "{best} over {} (est {} s{queue})", over.host, over.secs)
            }
        }
    }
}

/// Picks the host for a job from what the probes found. A new policy is a new type.
pub(crate) trait Placement {
    /// The host to run the job `ask` describes.
    fn choose(&self, ask: Ask<'_>, board: &[Snapshot]) -> Result<Choice, Error>;

    /// The typical times of `host` by key, as placement uses them; none for a policy without.
    fn medians(&self, _host: &HostName) -> Vec<(Key, Median)> {
        Vec::new()
    }
}

impl Ask<'_> {
    fn none(self, board: &[Snapshot]) -> Error {
        Error::NoHost { need: self.need, up: board.iter().map(|snap| snap.name.clone()).collect() }
    }

    /// The pinned host, when there is one: it must be up and fit.
    fn pinned(self, board: &[Snapshot]) -> Option<Result<Choice, Error>> {
        let pinned = self.pin?;
        Some(match board.iter().position(|snap| &snap.name == pinned) {
            Some(at) if board.get(at).is_some_and(|snap| self.need.fits(snap)) => {
                Ok(Choice { at, host: pinned.clone(), why: Why::Pinned })
            }
            Some(_) => Err(Error::NotGolden { host: pinned.clone() }),
            None => Err(self.none(board)),
        })
    }

    /// The hosts that may run the job and are quiet enough, with their places on the board; an
    /// error when hosts fit but every one is over the job's load ceiling.
    fn fits(self, board: &[Snapshot]) -> Result<Vec<(usize, &Snapshot)>, Error> {
        let fits: Vec<_> = board.iter().enumerate().filter(|(_, snap)| self.need.fits(snap)).collect();
        let Some(ceiling) = self.ceiling else { return Ok(fits) };
        let (calm, over): (Vec<_>, Vec<_>) = fits.into_iter().partition(|(_, snap)| snap.load <= ceiling);
        if calm.is_empty() && !over.is_empty() {
            return Err(Error::Loaded {
                ceiling,
                over: over.into_iter().map(|(_, snap)| (snap.name.clone(), snap.load)).collect(),
            });
        }
        Ok(calm)
    }
}
