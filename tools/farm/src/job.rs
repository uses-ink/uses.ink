//! A job: a command with what it needs, and what became of it. [`Flow`] reads a running job's
//! output line by line; [`Finished`] is the result a verb prints, as text or as `uses.farm/v1`.

use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::cli::Seconds;
use crate::event::{Event, Line};
use crate::exit::Exit;
use crate::host::Snapshot;
use crate::jobs::Recipe;
use crate::name::{HostName, JobId};
use crate::place::Need;
use crate::summary::Summary;

/// A command to run on a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    /// What it is called in the output: a stage, or the command.
    pub(crate) label: String,
    /// The command: `cargo`, then its arguments.
    pub(crate) words: Vec<String>,
    pub(crate) need: Need,
    /// Variables beyond the farm's own, as `NAME=value`; they win over them.
    pub(crate) env: Vec<String>,
    /// Its own hard limit, when the repo declares one.
    pub(crate) limit: Option<Seconds>,
}

impl From<Recipe> for Job {
    fn from(recipe: Recipe) -> Self {
        Self {
            label: recipe.name.to_string(),
            words: recipe.words,
            need: recipe.need,
            env: Vec::new(),
            limit: recipe.limit,
        }
    }
}

impl Job {
    /// The command as it is shown.
    pub(crate) fn shown(&self) -> String {
        self.words.join(" ")
    }

    /// The environment every farm run of `snap`'s host gets, the same every time so the host's
    /// `target/` stays warm: smaller debug info (line tables keep backtraces and panics readable,
    /// and the build and the link are faster), no colour, and the cores of one slot for cargo
    /// and for the test threads. `CARGO_TARGET_DIR` is never set.
    pub(crate) fn vars(&self, snap: &Snapshot) -> Vec<String> {
        self.vars_with(snap, std::env::var("NEXTEST_PROFILE").ok().as_deref())
    }

    /// [`Self::vars`] with the nextest profile the caller set, if any. The farm's hosts are
    /// shared, so a job runs the `ci` profile (the 100 ms budgets are for a quiet machine) unless
    /// the caller chose one.
    pub(crate) fn vars_with(&self, snap: &Snapshot, caller: Option<&str>) -> Vec<String> {
        let share = snap.share();
        let mut vars = vec![
            "CARGO_PROFILE_DEV_DEBUG=line-tables-only".to_owned(),
            "CARGO_TERM_COLOR=never".to_owned(),
            format!("CARGO_BUILD_JOBS={share}"),
            format!("NEXTEST_TEST_THREADS={share}"),
            "INSTA_UPDATE=no".to_owned(),
        ];
        if caller.is_none_or(str::is_empty) {
            vars.push("NEXTEST_PROFILE=ci".to_owned());
        }
        vars.extend(self.env.iter().cloned());
        vars
    }
}

/// How a call to run a job waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attach {
    /// Follow the job for at most this long, then let it go on without the client.
    Hold(Duration),
    /// Start it and leave.
    Detach,
}

/// Where a log is written as it arrives.
#[derive(Debug)]
pub(crate) struct LogFile {
    file: std::fs::File,
    failed: Option<std::io::Error>,
}

impl LogFile {
    pub(crate) fn create(path: &Path) -> Result<Self, std::io::Error> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Ok(Self { file: std::fs::File::create(path)?, failed: None })
    }

    fn put(&mut self, line: &str) {
        if self.failed.is_none() {
            self.failed = writeln!(self.file, "{line}").err();
        }
    }
}

/// A job's output and events, as they arrive.
#[derive(Debug, Default)]
pub(crate) struct Flow {
    pub(crate) slot: Option<u32>,
    pub(crate) queued: Duration,
    pub(crate) done: Option<(i32, Duration)>,
    pub(crate) aborted: bool,
    pub(crate) detached: bool,
    pub(crate) started: bool,
    /// The host never saw the job's body start.
    pub(crate) nobody: bool,
    /// The first line that carried the mark and was not an event.
    pub(crate) fault: Option<crate::event::Fault>,
    /// The job's own output.
    pub(crate) log: String,
    pub(crate) file: Option<LogFile>,
}

impl Flow {
    /// Takes one line of the call's output; returns the event it was, if it was one.
    pub(crate) fn feed(&mut self, text: &str) -> Option<Event> {
        match Line::read(text) {
            Ok(Line::Output(line)) => {
                self.log.push_str(&line);
                self.log.push('\n');
                if let Some(file) = &mut self.file {
                    file.put(&line);
                }
                None
            }
            Ok(Line::Event(event)) => {
                match &event {
                    Event::Slot { slot, queued } => (self.slot, self.queued) = (Some(*slot), *queued),
                    Event::Done { code, ran } => self.done = Some((*code, *ran)),
                    Event::Aborted => self.aborted = true,
                    Event::Detached => self.detached = true,
                    Event::Started => self.started = true,
                    Event::NoBody => self.nobody = true,
                    _ => {}
                }
                Some(event)
            }
            Err(fault) => {
                self.fault.get_or_insert(fault);
                None
            }
        }
    }

    /// How it ended, as far as the lines say.
    pub(crate) fn verdict(&self) -> Verdict {
        match (self.done, self.detached, self.started, self.aborted) {
            (Some((code, ran)), ..) => Verdict::Done { code, ran },
            (None, true, ..) | (None, false, true, _) => Verdict::Running,
            (None, false, false, true) => Verdict::Aborted,
            (None, false, false, false) => Verdict::Lost,
        }
    }

    /// The error that stopped the local log file, if one did.
    pub(crate) fn log_error(&mut self) -> Option<std::io::Error> {
        self.file.as_mut().and_then(|file| file.failed.take())
    }
}

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
}

impl Finished {
    pub(crate) fn exit(&self) -> Exit {
        self.verdict.exit()
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

/// `uses.farm/v1`: what `--json` prints for a job.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Wire {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) command: String,
    pub(crate) host: String,
    pub(crate) slot: Option<u32>,
    pub(crate) queued_secs: u64,
    /// `passed`, `failed`, `timed out`, `running`, `aborted` or `lost`.
    pub(crate) outcome: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) ran_secs: Option<u64>,
    pub(crate) log: String,
    pub(crate) summary: Summary,
}

/// The whole report of one command: `uses.farm/v1`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Document {
    pub(crate) schema: &'static str,
    /// The exit code the command ends with.
    pub(crate) exit: u8,
    pub(crate) jobs: Vec<Wire>,
    /// Files a bless changed, relative to the workspace.
    pub(crate) blessed: Vec<String>,
    /// Files a fmt changed, relative to the workspace.
    pub(crate) formatted: Vec<String>,
}

impl Document {
    pub(crate) const SCHEMA: &'static str = "uses.farm/v1";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(lines: &[&str]) -> Flow {
        let mut flow = Flow::default();
        for line in lines {
            flow.feed(line);
        }
        flow
    }

    #[test]
    fn a_finished_job_is_its_done_event_and_its_output() {
        let flow = feed(&["@@farm:slot 1 4", "   Compiling x", "@@farm:done 101 9"]);
        assert_eq!(flow.slot, Some(1));
        assert_eq!(flow.queued, Duration::from_secs(4));
        assert_eq!(flow.log, "   Compiling x\n");
        assert_eq!(flow.verdict(), Verdict::Done { code: 101, ran: Duration::from_secs(9) });
        assert_eq!(flow.verdict().exit(), Exit::Failed);
    }

    #[test]
    fn the_end_of_the_stream_without_a_result_is_lost_unless_something_says_otherwise() {
        assert_eq!(feed(&["@@farm:slot 0 0", "out"]).verdict(), Verdict::Lost);
        assert_eq!(feed(&["@@farm:slot 0 0", "@@farm:detached"]).verdict(), Verdict::Running);
        assert_eq!(feed(&["@@farm:aborted"]).verdict(), Verdict::Aborted);
        assert_eq!(feed(&["@@farm:started"]).verdict(), Verdict::Running);
        assert_eq!(feed(&[]).verdict().exit(), Exit::Farm);
        assert_eq!(feed(&["@@farm:done 124 1500"]).verdict().exit(), Exit::TimedOut);
    }

    #[test]
    fn a_bad_event_line_is_kept_not_swallowed() {
        let flow = feed(&["@@farm:wat"]);
        assert!(flow.fault.is_some());
    }

    #[test]
    fn the_environment_is_the_same_every_time_and_never_sets_the_target_dir() {
        let job = Job {
            label: "x".to_owned(),
            words: vec!["cargo".to_owned()],
            need: Need::Any,
            env: vec!["INSTA_UPDATE=always".to_owned()],
            limit: None,
        };
        let snap = crate::host::Snapshot::read(
            &crate::host::Host::local(),
            "now 1\narch x86_64\ncores 14\nload 0.1\nslots 2\nslot 0 free\nslot 1 free\nqueued 0\n",
        )
        .unwrap();
        let vars = job.vars_with(&snap, None);
        assert!(vars.contains(&"CARGO_BUILD_JOBS=7".to_owned()));
        assert!(vars.contains(&"NEXTEST_TEST_THREADS=7".to_owned()));
        assert!(vars.contains(&"CARGO_PROFILE_DEV_DEBUG=line-tables-only".to_owned()));
        assert_eq!(
            vars.last().map(String::as_str),
            Some("INSTA_UPDATE=always"),
            "the job's own variable comes last and wins"
        );
        assert!(!vars.iter().any(|var| var.starts_with("CARGO_TARGET_DIR")));
        assert_eq!(vars, job.vars_with(&snap, Some("")), "an empty profile counts as none");
        assert!(vars.contains(&"NEXTEST_PROFILE=ci".to_owned()), "shared hosts run the ci profile");
        let chosen = job.vars_with(&snap, Some("default"));
        assert!(!chosen.iter().any(|var| var.starts_with("NEXTEST_PROFILE")), "the caller's choice is left alone");
    }
}
