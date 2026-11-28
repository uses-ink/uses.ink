//! What a smoke stage can fail with.

use std::fmt;
use std::path::PathBuf;

use crate::process::Failure;

/// Why a stage of the smoke run failed.
#[derive(Debug)]
pub(crate) enum Fault {
    /// A command that did not end with success.
    Command { line: String, source: Failure },
    /// A file or folder of the run that could not be made, read or removed.
    Disk { path: PathBuf, source: std::io::Error },
    /// No free port could be found on the loopback address.
    Port(std::io::Error),
    /// The server could not be started.
    Start { line: String, source: std::io::Error },
    /// The server ended before it was listening.
    Exited { log: String },
    /// The server was running and never said it was listening.
    Unready { seconds: u64, log: String },
    /// A signed call stayed refused because the server was still quiet after its start.
    Quiet { line: String },
    /// An answer or a line of output that is not what the guide says.
    Expected { what: &'static str, wanted: String, found: String },
}

impl Fault {
    /// How much of what was found an error shows.
    const SHOWN: usize = 600;

    /// An answer that is not the one wanted, `found` cut to what fits.
    pub(super) fn expected(what: &'static str, wanted: impl Into<String>, found: &str) -> Self {
        let found = match found.char_indices().nth(Self::SHOWN) {
            Some((end, _)) => format!("{}...", found.get(..end).unwrap_or(found)),
            None => found.to_owned(),
        };
        Self::Expected { what, wanted: wanted.into(), found }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { line, .. } => write!(out, "`{line}` failed"),
            Self::Disk { path, .. } => write!(out, "cannot use {}", path.display()),
            Self::Port(_) => write!(out, "no free port on 127.0.0.1"),
            Self::Start { line, .. } => write!(out, "`{line}` could not be started"),
            Self::Exited { log } => write!(out, "the server ended before it was listening, saying:\n{log}"),
            Self::Unready { seconds, log } => {
                write!(out, "the server was not listening after {seconds} s, saying:\n{log}")
            }
            Self::Quiet { line } => {
                write!(out, "`{line}` was refused as the server was still quiet after its start, in every try")
            }
            Self::Expected { what, wanted, found } => write!(out, "{what}: wanted {wanted}, found:\n{found}"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Command { source, .. } => Some(source),
            Self::Disk { source, .. } | Self::Port(source) | Self::Start { source, .. } => Some(source),
            Self::Exited { .. } | Self::Unready { .. } | Self::Quiet { .. } | Self::Expected { .. } => None,
        }
    }
}
