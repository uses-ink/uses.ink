//! A child process run to completion, its failure kept whole: the exit status and stderr as
//! given, the I/O error as the source.

use std::fmt;
use std::io::Write as _;
use std::process::{Command, ExitStatus, Stdio};
use std::string::FromUtf8Error;

/// A command to run once, capturing its stdout and stderr.
#[derive(Debug)]
pub(crate) struct Process(Command);

impl From<Command> for Process {
    fn from(command: Command) -> Self {
        Self(command)
    }
}

impl Process {
    /// Runs it with `input` on stdin, and returns its stdout as text once it exits with success.
    pub(crate) fn text(mut self, input: &[u8]) -> Result<String, Failure> {
        let mut child = self
            .0
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Failure::Spawn)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input).map_err(Failure::Stdin)?;
        }
        let done = child.wait_with_output().map_err(Failure::Wait)?;
        if done.status.success() {
            String::from_utf8(done.stdout).map_err(Failure::Utf8)
        } else {
            Err(Failure::Exit { status: done.status, stderr: done.stderr })
        }
    }
}

/// How a child process failed.
#[derive(Debug)]
pub(crate) enum Failure {
    /// It could not be started.
    Spawn(std::io::Error),
    /// Its input could not be written.
    Stdin(std::io::Error),
    /// Waiting for it, or reading its output, failed.
    Wait(std::io::Error),
    /// It could not be stopped.
    Kill(std::io::Error),
    /// A thread reading its output panicked.
    Panicked { thread: &'static str },
    /// It exited without success.
    Exit { status: ExitStatus, stderr: Vec<u8> },
    /// Its output is not UTF-8.
    Utf8(FromUtf8Error),
    /// It was stopped on purpose, because the run it belongs to was cancelled.
    Cancelled,
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(_) => write!(out, "it could not be started"),
            Self::Stdin(_) => write!(out, "its input could not be written"),
            Self::Wait(_) => write!(out, "its output could not be read"),
            Self::Kill(_) => write!(out, "it could not be stopped"),
            Self::Panicked { thread } => write!(out, "the thread reading its {thread} panicked"),
            Self::Exit { status, stderr } => {
                write!(out, "it ended with {status}")?;
                match String::from_utf8_lossy(stderr).trim_end() {
                    "" => Ok(()),
                    text => write!(out, ", saying:\n{text}"),
                }
            }
            Self::Utf8(_) => write!(out, "its output is not UTF-8"),
            Self::Cancelled => write!(out, "it was cancelled"),
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(source) | Self::Stdin(source) | Self::Wait(source) | Self::Kill(source) => Some(source),
            Self::Exit { .. } | Self::Panicked { .. } | Self::Cancelled => None,
            Self::Utf8(source) => Some(source),
        }
    }
}
