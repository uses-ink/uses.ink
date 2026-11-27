//! What is wrong with the job file, and where.

use std::fmt;

use crate::cache::inputs::glob;
use crate::cli::error::Listed;
use crate::cli::name::JobName;

/// A line of the job file, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Line(pub(crate) usize);

impl Line {
    /// The line of `word` in the table of job `name`, else the table's own line, else the first.
    pub(super) fn of(text: &str, name: &JobName, word: &str) -> Self {
        let header = format!("[job.{name}]");
        let mut table = None;
        for (at, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed == header {
                table = Some(at);
            } else if trimmed.starts_with('[') && table.is_some() {
                break;
            } else if table.is_some() && line.contains(word) {
                return Self(at + 1);
            }
        }
        Self(table.map_or(1, |at| at + 1))
    }
}

impl fmt::Display for Line {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "line {}", self.0)
    }
}

/// A job file that cannot be used, or a job it does not declare.
#[derive(Debug)]
pub(crate) enum Fault {
    Read(std::io::Error),
    Toml(Box<toml::de::Error>),
    /// A job name the farm does not allow.
    Name(crate::cli::name::Fault),
    /// A `[job.<name>]` table that is not one.
    Entry {
        name: JobName,
        source: Box<toml::de::Error>,
    },
    /// A glob of `inputs` that is not one.
    Inputs {
        name: JobName,
        at: Line,
        source: glob::Fault,
    },
    /// An `inputs = []`: it would read nothing.
    NoInputs {
        name: JobName,
        at: Line,
    },
    /// A file with no `[job.*]`.
    Empty,
    /// A job asked for that the file does not declare.
    Unknown {
        given: JobName,
        known: Vec<JobName>,
    },
}

impl Fault {
    /// Whether the person named a job that is not there: a usage error, not the farm's.
    pub(crate) const fn is_usage(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => write!(out, "cannot be read"),
            Self::Toml(_) => write!(out, "is not the TOML expected"),
            Self::Name(_) => write!(out, "names a job that is not allowed"),
            Self::Entry { name, .. } => {
                write!(
                    out,
                    "has a `[job.{name}]` that is not `run`, `golden` and the optional `limit`, `inputs` and `cache`"
                )
            }
            Self::Inputs { name, at, .. } => {
                write!(out, "has an `inputs` glob of `[job.{name}]` that is not allowed, at {at}")
            }
            Self::NoInputs { name, at } => {
                write!(out, "has an empty `inputs` in `[job.{name}]` at {at}; leave it out to read the whole tree")
            }
            Self::Empty => write!(out, "declares no `[job.<name>]`"),
            Self::Unknown { given, known } => write!(out, "declares no job `{given}`; the jobs are {}", Listed(known)),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) => Some(source),
            Self::Toml(source) | Self::Entry { source, .. } => Some(source),
            Self::Name(source) => Some(source),
            Self::Inputs { source, .. } => Some(source),
            Self::Empty | Self::NoInputs { .. } | Self::Unknown { .. } => None,
        }
    }
}
