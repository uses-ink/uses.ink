//! Starts one process from an argument vector and follows it to its end.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::cli::console::Console;
use crate::host::transport::Done;
use crate::host::transport::process::Failure;
use crate::host::transport::program::Stdin;
use crate::job::quote::Line;

/// What goes to a process's standard input.
#[derive(Debug, Clone, Copy)]
pub(super) enum Feed<'a> {
    Nothing,
    /// Open until the call ends or the time is up, when `detach` is written and it is closed.
    Open(Duration),
    Text(&'a str),
}

impl From<Stdin> for Feed<'_> {
    fn from(stdin: Stdin) -> Self {
        match stdin {
            Stdin::Null => Self::Nothing,
            Stdin::Held(until) => Self::Open(until),
        }
    }
}

/// Starts processes, saying each command first when asked to.
#[derive(Debug, Clone)]
pub(crate) struct Exec {
    console: Arc<Console>,
}

/// How long a process gets to end after `detach`, before it is killed.
const GRACE: Duration = Duration::from_secs(10);

/// How long a wait with no deadline sleeps before it asks again: a call with no limit still
/// wakes once an hour, so a dead one is noticed.
const UNBOUNDED_WAIT: Duration = Duration::from_secs(3600);

impl Exec {
    pub(crate) fn new(console: Arc<Console>) -> Self {
        Self { console }
    }

    /// Runs `argv`, each line of its stdout into `lines`.
    pub(super) fn spawn(&self, argv: &[String], feed: Feed<'_>, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        self.console.show(Line(argv));
        let (first, rest) =
            argv.split_first().ok_or_else(|| Failure::Spawn(std::io::ErrorKind::InvalidInput.into()))?;
        let mut command = Command::new(first);
        command.args(rest).stdout(Stdio::piped()).stderr(Stdio::piped());
        command.stdin(match feed {
            Feed::Nothing => Stdio::null(),
            Feed::Open(_) | Feed::Text(_) => Stdio::piped(),
        });
        let mut child = command.spawn().map_err(Failure::Spawn)?;
        if let Feed::Text(text) = feed {
            child.stdin.take().map_or(Ok(()), |mut stdin| stdin.write_all(text.as_bytes())).map_err(Failure::Stdin)?;
        }
        Follow::new(&mut child, feed).run(lines)
    }
}

/// A running child being read until it ends.
struct Follow<'a> {
    child: &'a mut Child,
    deadline: Option<Instant>,
    detaching: Option<Instant>,
    /// The first thing that went wrong while following; it ends the call once the child is reaped.
    fault: Option<Failure>,
}

impl<'a> Follow<'a> {
    fn new(child: &'a mut Child, feed: Feed<'_>) -> Self {
        let deadline = match feed {
            Feed::Open(until) => Some(Instant::now() + until),
            Feed::Nothing | Feed::Text(_) => None,
        };
        Self { child, deadline, detaching: None, fault: None }
    }

    fn run(mut self, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        let (receiver, reading) = Self::lines(self.child.stdout.take());
        let errors = Self::errors(self.child.stderr.take());
        let mut writer = self.child.stdin.take();
        loop {
            match receiver.recv_timeout(self.next_wait()) {
                Ok(line) => lines(&line),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            // checked after every line too: a job that never goes quiet still has its time
            if self.due()
                && let Err(failure) = self.expire(&mut writer)
            {
                self.fail(failure);
            }
        }
        let read = reading.join().map_err(|_| Failure::Panicked { thread: "output" });
        let status = self.child.wait().map_err(Failure::Wait);
        let stderr = errors.join().map_err(|_| Failure::Panicked { thread: "errors" });
        if let Some(failure) = self.fault.take() {
            return Err(failure);
        }
        read?.map_err(Failure::Wait)?;
        Ok(Done { status: status?, stderr: stderr?.map_err(Failure::Wait)? })
    }

    /// Whether the time of the next step has come.
    fn due(&self) -> bool {
        self.detaching.or(self.deadline).is_some_and(|at| Instant::now() >= at)
    }

    /// The next step: say `detach` and close the input, or, when the grace is over, kill.
    fn expire(&mut self, writer: &mut Option<ChildStdin>) -> Result<(), Failure> {
        if let Some(mut input) = writer.take() {
            // the job outlives us from here: the host side sees `detach`, then the end of input
            if let Err(source) = input.write_all(b"detach\n") {
                // a child that has already ended has no one to tell
                if self.child.try_wait().map_err(Failure::Wait)?.is_none() {
                    return Err(Failure::Stdin(source));
                }
            }
            self.detaching = Some(Instant::now() + GRACE);
        } else if self.detaching.is_some() {
            self.stop();
            self.child.kill().map_err(Failure::Kill)?;
        }
        Ok(())
    }

    /// Ends the call with `failure`, unless one came first: the child is killed.
    fn fail(&mut self, failure: Failure) {
        self.stop();
        let killed = self.child.kill().map_err(Failure::Kill);
        self.fault.get_or_insert(failure);
        if let Err(failure) = killed {
            self.fault.get_or_insert(failure);
        }
    }

    /// No more steps are due.
    fn stop(&mut self) {
        self.deadline = None;
        self.detaching = None;
    }

    /// A thread that reads `stdout` line by line into the channel.
    fn lines(stdout: Option<ChildStdout>) -> (mpsc::Receiver<String>, thread::JoinHandle<std::io::Result<()>>) {
        let (sender, receiver) = mpsc::channel();
        let reading = thread::spawn(move || {
            let mut reader = BufReader::new(Self::source(stdout));
            let mut raw = Vec::new();
            while reader.read_until(b'\n', &mut raw)? > 0 {
                let line = String::from_utf8_lossy(&raw).trim_end_matches(['\n', '\r']).to_owned();
                raw.clear();
                if sender.send(line).is_err() {
                    break;
                }
            }
            Ok(())
        });
        (receiver, reading)
    }

    /// A thread that collects what `stderr` says, the first megabyte of it.
    fn errors(stderr: Option<ChildStderr>) -> thread::JoinHandle<std::io::Result<String>> {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            Self::source(stderr).take(1 << 20).read_to_end(&mut bytes)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
    }

    fn source(pipe: Option<impl std::io::Read + Send + 'static>) -> Box<dyn std::io::Read + Send> {
        match pipe {
            Some(pipe) => Box::new(pipe),
            None => Box::new(std::io::empty()),
        }
    }

    /// How long to wait for the next line: until the deadline, or the end of the grace after it.
    fn next_wait(&self) -> Duration {
        let limit = self.detaching.or(self.deadline);
        limit.map_or(UNBOUNDED_WAIT, |at| at.saturating_duration_since(Instant::now()).max(Duration::from_millis(10)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec() -> Exec {
        Exec::new(Arc::new(Console::new(false)))
    }

    fn sh(script: &str) -> Vec<String> {
        ["sh", "-c", script].map(str::to_owned).to_vec()
    }

    #[test]
    fn a_detach_that_cannot_be_written_to_a_live_child_is_a_typed_failure() {
        let done = exec().spawn(&sh("exec 0<&-; exec sleep 5"), Feed::Open(Duration::from_millis(30)), &mut |_| {});
        assert!(matches!(done, Err(Failure::Stdin(_))), "{done:?}");
    }

    #[test]
    fn stderr_that_is_not_utf8_is_kept_lossily_not_dropped() {
        let done = exec().spawn(&sh("printf 'a\\377b' >&2"), Feed::Nothing, &mut |_| {}).unwrap();
        assert_eq!(done.stderr, "a\u{fffd}b");
    }
}
