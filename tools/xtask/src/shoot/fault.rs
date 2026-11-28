//! What a run of `cargo xtask shoot` can fail with.

use std::fmt;
use std::num::ParseIntError;
use std::path::PathBuf;

use crate::process::Failure;

/// One thing that went wrong taking pictures.
#[derive(Debug)]
pub(crate) enum Fault {
    /// A width that is not a number.
    Width { given: String, source: ParseIntError },
    /// A list of widths that is empty, or has one the browser cannot make.
    Range { given: String },
    /// A flag that does not exist, or one that lacks its value.
    Flag { given: String },
    /// A command of the run that failed.
    Command { line: String, source: Failure },
    /// A server or the browser could not be started.
    Spawn { program: String, source: std::io::Error },
    /// A file or folder the run needs is missing or unreadable.
    File { path: PathBuf, source: std::io::Error },
    /// The plan for the browser could not be written as JSON.
    Plan(serde_json::Error),
    /// A server that ended before it listened.
    Exited { name: &'static str, log: String },
    /// A server that was running and never listened.
    Unready { name: &'static str, seconds: u64, log: String },
    /// A decision file that the pictures name and that is not in `docs/decisions/`.
    Decision { number: &'static str },
    /// A folder given as the mock that holds no `index.html`.
    Mock { path: PathBuf },
    /// `package-lock.json` is missing: `cargo xtask shoot lock` prints it, and it is committed.
    Lock { path: PathBuf },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Width { given, .. } => write!(out, "`{given}` is not a width in pixels"),
            Self::Range { given } => {
                write!(out, "`--width {given}` must list from one to eight widths, each from 200 to 4000")
            }
            Self::Flag { given } => {
                write!(out, "no flag `{given}`, or it has no value; the flags are `--width 1440,390` and `--from <folder>`")
            }
            Self::Command { line, .. } => write!(out, "`{line}` failed"),
            Self::Spawn { program, .. } => {
                write!(out, "`{program}` could not be started; is node installed for this host?")
            }
            Self::File { path, .. } => write!(out, "cannot use {}", path.display()),
            Self::Plan(_) => out.write_str("the plan for the browser cannot be written as JSON"),
            Self::Exited { name, log } => write!(out, "the {name} server ended before it listened, saying:\n{log}"),
            Self::Unready { name, seconds, log } => {
                write!(out, "the {name} server did not listen in {seconds} s, saying:\n{log}")
            }
            Self::Decision { number } => write!(out, "no decision {number} in docs/decisions/ to take a picture of"),
            Self::Mock { path } => write!(out, "{} has no index.html: `--from` names the mock's folder", path.display()),
            Self::Lock { path } => {
                write!(out, "{} is missing: `cargo xtask shoot lock` prints it, and it is committed", path.display())
            }
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Width { source, .. } => Some(source),
            Self::Command { source, .. } => Some(source),
            Self::Spawn { source, .. } | Self::File { source, .. } => Some(source),
            Self::Plan(source) => Some(source),
            Self::Range { .. }
            | Self::Flag { .. }
            | Self::Exited { .. }
            | Self::Unready { .. }
            | Self::Decision { .. }
            | Self::Mock { .. }
            | Self::Lock { .. } => None,
        }
    }
}
