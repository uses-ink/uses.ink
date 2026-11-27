//! What `farm status` says about placement: the timings each host is judged by, and where each
//! declared job would go now. Printed as lines, or as JSON with `--json`.

use std::fmt;

use serde::Serialize;

use crate::cli::error::Chain;
use crate::host::fleet::Probed;
use crate::host::{Host, Snapshot};
use crate::job::Job;
use crate::place::{Ask, Placement};

/// The wire form of `farm status --json`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Overview {
    pub(crate) schema: &'static str,
    pub(crate) hosts: Vec<Row>,
    /// Where each declared job would be placed with the hosts as they are now.
    pub(crate) next: Vec<Next>,
    /// Hosts that stop themselves and do not answer: not an error.
    pub(crate) asleep: Vec<Sleeping>,
    pub(crate) down: Vec<Gone>,
}

/// A host that is asleep.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Sleeping {
    pub(crate) host: String,
    /// Seconds waking it adds to its expected finish.
    pub(crate) wake_cost: u64,
    /// Its shape when last up; none if it never was, and then only `--host` places on it.
    pub(crate) arch: Option<String>,
    pub(crate) cores: Option<u32>,
}

/// One host that answered.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Row {
    pub(crate) name: String,
    pub(crate) arch: String,
    pub(crate) golden: bool,
    pub(crate) cores: u32,
    pub(crate) slots: u32,
    pub(crate) busy: u32,
    pub(crate) queued: u32,
    pub(crate) load: String,
    /// Median pass seconds by job key: what placement uses.
    pub(crate) medians: Vec<Typical>,
}

/// The typical time of one key on one host.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Typical {
    pub(crate) key: String,
    pub(crate) secs: u64,
    pub(crate) runs: u32,
}

/// Where a job would go.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Next {
    pub(crate) job: String,
    pub(crate) host: Option<String>,
    /// The one-line explanation, or why no host fits.
    pub(crate) why: String,
}

/// A host that did not answer.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Gone {
    pub(crate) host: String,
    pub(crate) error: String,
}

impl Overview {
    pub(crate) const SCHEMA: &'static str = "uses.farm/status/v1";

    /// Reads the hosts as probed and as placement sees them (`board`), through `placement`, for
    /// `jobs`.
    pub(crate) fn of(probed: &Probed, board: &[Snapshot], placement: &impl Placement, jobs: &[Job]) -> Self {
        Self {
            schema: Self::SCHEMA,
            hosts: probed.up.iter().map(|snap| Row::of(snap, placement)).collect(),
            next: jobs.iter().map(|job| Next::of(job, board, placement)).collect(),
            asleep: probed.asleep.iter().map(|host| Sleeping::of(host, board)).collect(),
            down: probed
                .down
                .iter()
                .map(|(host, error)| Gone { host: host.to_string(), error: Chain(error).to_string() })
                .collect(),
        }
    }
}

impl Row {
    fn of(snap: &Snapshot, placement: &impl Placement) -> Self {
        Self {
            name: snap.name.to_string(),
            arch: snap.arch.to_string(),
            golden: snap.golden,
            cores: snap.cores,
            slots: snap.slots,
            busy: snap.busy,
            queued: snap.queued,
            load: snap.load.to_string(),
            medians: placement
                .medians(&snap.name)
                .into_iter()
                .map(|(key, median)| Typical { key: key.to_string(), secs: median.secs, runs: median.runs })
                .collect(),
        }
    }
}

impl Next {
    fn of(job: &Job, board: &[Snapshot], placement: &impl Placement) -> Self {
        let asked = Ask { need: job.need, key: &job.key, pin: None };
        match placement.choose(asked, board) {
            Ok(choice) => Self { job: job.label.clone(), host: Some(choice.host.to_string()), why: choice.to_string() },
            Err(error) => Self { job: job.label.clone(), host: None, why: Chain(&error).to_string() },
        }
    }
}

impl Sleeping {
    fn of(host: &Host, board: &[Snapshot]) -> Self {
        let seen = board.iter().find(|snap| snap.name == host.name);
        Self {
            host: host.name.to_string(),
            wake_cost: host.wake.as_ref().map_or(0, |wake| wake.cost.0),
            arch: seen.map(|snap| snap.arch.to_string()),
            cores: seen.map(|snap| snap.cores),
        }
    }
}

impl fmt::Display for Sleeping {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cost = self.wake_cost;
        match (&self.arch, self.cores) {
            (Some(arch), Some(cores)) => write!(
                out,
                "{}: asleep ({arch}, {cores} cores as last seen); placement adds {cost} s to wake it, so it is used only when that still finishes first",
                self.host
            ),
            _ => write!(
                out,
                "{}: asleep, never seen up; waking costs about {cost} s; `farm --host {} <verb>` wakes it once so placement knows its size",
                self.host, self.host
            ),
        }
    }
}

impl fmt::Display for Row {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.medians.is_empty() {
            return write!(out, "{}: no timings yet, so placement tries it when a slot is free", self.name);
        }
        write!(out, "{}: median pass seconds by job:", self.name)?;
        for (at, typical) in self.medians.iter().enumerate() {
            let comma = if at == 0 { "" } else { "," };
            write!(out, "{comma} {} {} s ({} runs)", typical.key, typical.secs, typical.runs)?;
        }
        Ok(())
    }
}

impl fmt::Display for Next {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "next {}: {}", self.job, self.why)
    }
}
