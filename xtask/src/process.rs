//! Child processes run to completion, their failures kept whole: the exit status and stderr as
//! given, the I/O and parse errors as sources.

use std::fmt;
use std::io::Write as _;
use std::process::{Command, ExitStatus, Stdio};
use std::string::FromUtf8Error;

use serde::de::DeserializeOwned;

/// A command to run once, capturing its stdout and stderr.
#[derive(Debug)]
pub(crate) struct Process(Command);

impl From<Command> for Process {
    fn from(command: Command) -> Self {
        Self(command)
    }
}

impl Process {
    /// Runs it with `input` on stdin, and returns its stdout once it exits with success.
    pub(crate) fn stdout(mut self, input: &[u8]) -> Result<Vec<u8>, Failure> {
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
            Ok(done.stdout)
        } else {
            Err(Failure::Exit { status: done.status, stderr: done.stderr })
        }
    }

    /// Its stdout, as UTF-8 text.
    pub(crate) fn text(self, input: &[u8]) -> Result<String, Failure> {
        String::from_utf8(self.stdout(input)?).map_err(Failure::Utf8)
    }

    /// Its stdout, as JSON.
    pub(crate) fn json<T: DeserializeOwned>(self) -> Result<T, Failure> {
        serde_json::from_slice(&self.stdout(&[])?).map_err(Failure::Json)
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
    /// It exited without success.
    Exit { status: ExitStatus, stderr: Vec<u8> },
    /// Its output is not UTF-8.
    Utf8(FromUtf8Error),
    /// Its output is not the JSON expected.
    Json(serde_json::Error),
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(_) => write!(out, "it could not be started"),
            Self::Stdin(_) => write!(out, "its input could not be written"),
            Self::Wait(_) => write!(out, "its output could not be read"),
            Self::Exit { status, stderr } => {
                write!(out, "it ended with {status}")?;
                match String::from_utf8_lossy(stderr).trim_end() {
                    "" => Ok(()),
                    text => write!(out, ", saying:\n{text}"),
                }
            }
            Self::Utf8(_) => write!(out, "its output is not UTF-8"),
            Self::Json(_) => write!(out, "its output is not the JSON expected"),
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(source) | Self::Stdin(source) | Self::Wait(source) => Some(source),
            Self::Exit { .. } => None,
            Self::Utf8(source) => Some(source),
            Self::Json(source) => Some(source),
        }
    }
}
