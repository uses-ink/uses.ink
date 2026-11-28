//! What a run of a Worker can fail with.

use std::fmt;
use std::path::PathBuf;

use crate::process::Failure;

/// A response that is not HTTP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wire {
    /// No end of the head.
    Head,
    /// No status code.
    Status,
    /// A chunk that is cut or has no size.
    Chunk,
}

impl fmt::Display for Wire {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Head => "the head of the response never ended",
            Self::Status => "the response has no status",
            Self::Chunk => "a chunk of the response is cut",
        })
    }
}

impl std::error::Error for Wire {}

/// One thing that went wrong running a Worker.
#[derive(Debug)]
pub(crate) enum Fault {
    /// A command of the run failed.
    Command { program: String, source: Failure },
    /// A command could not be started.
    Spawn { program: String, source: std::io::Error },
    /// A file the run needs is missing or unreadable.
    File { path: PathBuf, source: std::io::Error },
    /// `package-lock.json` is missing: `cargo xtask worker lock` writes it.
    Lock { path: PathBuf },
    /// `wrangler dev` did not say it was ready.
    Ready { log: String },
    /// The Worker could not be reached.
    Connect(std::io::Error),
    /// A call could not be sent or its answer read.
    Send { path: String, wire: Wire, source: std::io::Error },
    /// The answer is not HTTP.
    Wire { path: String, wire: Wire },
    /// The Worker answered with a status that is not 200.
    Status { path: String, status: u16, body: String, log: String },
    /// What wrangler printed as JSON is not.
    Json { what: String, source: serde_json::Error },
    /// The schema or the ledger wrangler made is not the one SQLite made.
    Differs { expected: String, found: String },
    /// The Worker said no to something it checks.
    Refused { path: String, line: String },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { program, .. } => write!(out, "`{program}` failed"),
            Self::Spawn { program, .. } => {
                write!(out, "`{program}` could not be started; is node installed for this host?")
            }
            Self::File { path, .. } => write!(out, "cannot use {}", path.display()),
            Self::Lock { path } => {
                write!(out, "{} is missing: `cargo xtask worker lock` prints it, and it is committed", path.display())
            }
            Self::Ready { log } => write!(out, "`wrangler dev` never said it was ready; its output:\n{log}"),
            Self::Connect(_) => out.write_str("the Worker cannot be reached on this machine"),
            Self::Send { path, wire, .. } => write!(out, "the call to {path} could not be completed ({wire})"),
            Self::Wire { path, wire } => write!(out, "the answer of {path} is not HTTP: {wire}"),
            Self::Status { path, status, body, log } => {
                write!(out, "{path} answered {status}:\n{body}\nthe Worker's output:\n{log}")
            }
            Self::Json { what, .. } => write!(out, "wrangler's answer to {what} is not JSON"),
            Self::Differs { expected, found } => {
                write!(out, "D1 made another schema than SQLite.\nSQLite:\n{expected}\nD1:\n{found}")
            }
            Self::Refused { path, line } => write!(out, "{path} says no: {line}"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Command { source, .. } => Some(source),
            Self::Spawn { source, .. }
            | Self::File { source, .. }
            | Self::Connect(source)
            | Self::Send { source, .. } => Some(source),
            Self::Wire { wire, .. } => Some(wire),
            Self::Json { source, .. } => Some(source),
            Self::Lock { .. }
            | Self::Ready { .. }
            | Self::Status { .. }
            | Self::Refused { .. }
            | Self::Differs { .. } => None,
        }
    }
}
