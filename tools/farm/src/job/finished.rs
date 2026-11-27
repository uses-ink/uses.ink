//! What became of a job: [`Finished`] is the result a verb prints, and [`Totals`] adds them up.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::exit::Exit;
use crate::cli::name::{HostName, JobId};
use crate::job::Served;
use crate::job::document::Wire;
use crate::job::summary::Summary;
/// How a job ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The command ended with `code`.
    Done { code: i32, ran: Duration },
    /// Still going on its host.
    Running,
    /// Stopped with its client.
    Aborted,
    /// The connection ended and nothing says how the job did.
    Lost,
}

impl Verdict {
    pub(crate) const fn exit(self) -> Exit {
        match self {
            Self::Done { code, .. } => Exit::of(code),
            Self::Running => Exit::Running,
            Self::Aborted | Self::Lost => Exit::Farm,
        }
    }
}

/// A job that has ended, or been handed over, and what it printed.
#[derive(Debug, Clone)]
pub(crate) struct Finished {
    pub(crate) id: JobId,
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) host: HostName,
    pub(crate) slot: Option<u32>,
    pub(crate) queued: Duration,
    pub(crate) verdict: Verdict,
    pub(crate) log: PathBuf,
    pub(crate) summary: Summary,
    pub(crate) served: Served,
}

impl Finished {
    pub(crate) fn exit(&self) -> Exit {
        self.verdict.exit()
    }

    /// Seconds the command ran, when it ended.
    pub(crate) const fn ran(&self) -> u64 {
        match self.verdict {
            Verdict::Done { ran, .. } => ran.as_secs(),
            Verdict::Running | Verdict::Aborted | Verdict::Lost => 0,
        }
    }

    /// The machine form.
    pub(crate) fn wire(&self) -> Wire {
        let (outcome, code, ran) = match self.verdict {
            Verdict::Done { code, ran } => (self.exit().to_string(), Some(code), Some(ran.as_secs())),
            Verdict::Running => ("running".to_owned(), None, None),
            Verdict::Aborted => ("aborted".to_owned(), None, None),
            Verdict::Lost => ("lost".to_owned(), None, None),
        };
        Wire {
            id: self.id.to_string(),
            label: self.label.clone(),
            command: self.command.clone(),
            host: self.host.to_string(),
            slot: self.slot,
            queued_secs: self.queued.as_secs(),
            outcome,
            exit_code: code,
            ran_secs: ran,
            log: self.log.display().to_string(),
            summary: self.summary.clone(),
            served: self.served.word(),
        }
    }
}

impl fmt::Display for Finished {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slot = self.slot.map_or_else(String::new, |slot| format!(" slot {slot},"));
        write!(out, "== {}: {},{slot} queued {} s", self.label, self.host, self.queued.as_secs())?;
        match self.verdict {
            Verdict::Done { ran, .. } => write!(out, ", ran {} s", ran.as_secs())?,
            Verdict::Running | Verdict::Aborted | Verdict::Lost => {}
        }
        writeln!(out, "\n   $ {}", self.command)?;
        match self.served {
            Served::Cached { ago } => {
                writeln!(out, "passed earlier on this tree ({}, {ago} ago)\nlog {}", self.id, self.log.display())?;
                return write!(out, "{}", self.summary);
            }
            Served::Attached => writeln!(out, "attached to the identical job already running, {}", self.id)?,
            Served::Ran => {}
        }
        match self.verdict {
            Verdict::Done { code, .. } => {
                writeln!(out, "{} (exit {code}), log {}", self.exit().to_string().to_uppercase(), self.log.display())?;
            }
            Verdict::Running => writeln!(out, "RUNNING as {}; continue with `farm wait {}`", self.id, self.id)?,
            Verdict::Aborted => writeln!(out, "ABORTED on the host")?,
            Verdict::Lost => writeln!(out, "LOST: the connection ended and no result came")?,
        }
        write!(out, "{}", self.summary)
    }
}

/// What a `farm check` did, in the one line it ends with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Totals {
    pub(crate) ran: u32,
    pub(crate) cached: u32,
    pub(crate) attached: u32,
    /// Seconds the stages that ran spent on their hosts.
    pub(crate) secs: u64,
}

impl Totals {
    pub(crate) fn of(done: &[Finished]) -> Self {
        let mut totals = Self::default();
        for job in done {
            match job.served {
                Served::Ran => {
                    totals.ran += 1;
                    totals.secs += job.ran();
                }
                Served::Cached { .. } => totals.cached += 1,
                Served::Attached => totals.attached += 1,
            }
        }
        totals
    }
}

impl fmt::Display for Totals {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "check: {} stages run, {} served from cache, {} deduped, {} host seconds",
            self.ran, self.cached, self.attached, self.secs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Ago;

    #[test]
    fn the_check_summary_counts_run_cached_and_deduped_stages_and_host_seconds() {
        let done = |served, secs| Finished {
            id: "petal.1a".parse().unwrap(),
            label: "x".to_owned(),
            command: "c".to_owned(),
            host: "petal".parse().unwrap(),
            slot: None,
            queued: Duration::ZERO,
            verdict: Verdict::Done { code: 0, ran: Duration::from_secs(secs) },
            log: PathBuf::new(),
            summary: Summary::default(),
            served,
        };
        let all = [
            done(Served::Ran, 40),
            done(Served::Ran, 2),
            done(Served::Cached { ago: Ago(Duration::from_secs(5)) }, 900),
            done(Served::Attached, 700),
        ];
        assert_eq!(
            Totals::of(&all).to_string(),
            "check: 2 stages run, 1 served from cache, 1 deduped, 42 host seconds"
        );
    }
}
