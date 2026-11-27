//! A running job: [`Flow`] reads its output line by line.

use std::io::Write as _;
use std::path::Path;
use std::time::Duration;

use crate::cli::name::HostName;
use crate::cli::name::JobId;
use crate::job::Verdict;
use crate::job::event::{Event, Line};

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

/// What the host said about a job's body.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    #[default]
    Unsaid,
    Started,
    /// It was not seen to start in time: the job may be running unobserved.
    Missing,
}

/// A job's output and events, as they arrive.
#[derive(Debug, Default)]
pub(crate) struct Flow {
    pub(crate) slot: Option<u32>,
    pub(crate) queued: Duration,
    pub(crate) done: Option<(i32, Duration)>,
    pub(crate) aborted: bool,
    pub(crate) detached: bool,
    /// What the host said about the job's body.
    pub(crate) body: Body,
    /// The first line that carried the mark and was not an event.
    pub(crate) fault: Option<crate::job::event::Fault>,
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
                    Event::Started => self.body = Body::Started,
                    Event::NoBody => self.body = Body::Missing,
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
        match (self.done, self.detached, self.body == Body::Started, self.aborted) {
            (Some((code, ran)), ..) => Verdict::Done { code, ran },
            (None, true, ..) | (None, false, true, _) => Verdict::Running,
            (None, false, false, true) => Verdict::Aborted,
            (None, false, false, false) => Verdict::Lost,
        }
    }

    /// The error for a job that ended with no word: its body never showed, or the connection ended.
    pub(crate) fn lost(&self, job: JobId) -> crate::cli::error::Error {
        match self.body {
            Body::Missing => crate::cli::error::Error::NoBody { job },
            Body::Unsaid | Body::Started => crate::cli::error::Error::Lost { job },
        }
    }

    /// How the job ended, or the error that stopped it on the way: a fault the host said, the
    /// local log file failing, or a connection that ended with no word.
    pub(crate) fn settle(
        &mut self,
        host: &HostName,
        id: &JobId,
        log: &Path,
    ) -> Result<Verdict, crate::cli::error::Error> {
        if let Some(source) = self.fault.take() {
            return Err(crate::cli::error::Error::Said { host: host.clone(), source });
        }
        if let Some(source) = self.log_error() {
            return Err(crate::cli::error::Error::Io { path: log.to_path_buf(), source });
        }
        match self.verdict() {
            Verdict::Lost => Err(self.lost(id.clone())),
            verdict => Ok(verdict),
        }
    }

    /// The error that stopped the local log file, if one did.
    pub(crate) fn log_error(&mut self) -> Option<std::io::Error> {
        self.file.as_mut().and_then(|file| file.failed.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::exit::Exit;

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
}
