//! Which crates a change affects: the files changed in the workspace since a base, mapped to the
//! crates that own them, widened by the crates that depend on those, so `farm test` runs only
//! what the change can break. The farm knows no project: the crates come from `cargo metadata`
//! on a host, the changed files from jj, and what counts as irrelevant from the job file.
//!
//! - [`graph::Graph`]: the members and the edges between them.
//! - [`touched::Touch`]: the seam to the version control; [`touched::Diff`] asks jj.
//! - [`choice::Choice`]: the crates chosen, or all, or none, and the line that says why.

mod choice;
mod graph;
mod touched;

#[cfg(test)]
mod tests;

use std::fmt;
use std::path::PathBuf;

use crate::cache::inputs::glob::Selection;
use crate::cli::error::Error;
use crate::cli::exit::Exit;
use crate::host::transport::process::Failure;
pub(crate) use choice::Choice;
use graph::{Graph, Package};
pub(crate) use touched::{BadBase, Base, Diff, Touch};

/// What the job file says about narrowing: where to measure from, and the paths that no test
/// reads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Scope {
    /// The revision to measure from, when the file names one.
    pub(crate) base: Option<Base>,
    /// Globs of paths in no crate that cannot affect a test: prose, licences.
    pub(crate) ignore: Selection,
}

/// The seam to the host: the JSON `cargo metadata --no-deps --format-version 1` prints for the
/// workspace.
pub(crate) trait Layout {
    /// Runs it in the synced workspace on a host and returns what it printed.
    fn json(&self) -> Result<String, Fault>;
}

/// The question: which crates does this workspace's change affect.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Affected<'a, L, T> {
    pub(crate) layout: &'a L,
    pub(crate) touch: &'a T,
    pub(crate) scope: &'a Scope,
}

impl<L: Layout, T: Touch> Affected<'_, L, T> {
    /// The crates affected by what changed since `base`.
    pub(crate) fn choose(&self, base: &Base) -> Result<Choice, Fault> {
        let graph = Graph::read(&self.layout.json()?)?;
        let touched = self.touch.paths(base)?;
        Ok(Choice::of(&graph, &touched, &self.scope.ignore))
    }
}

/// Why the affected crates could not be found.
#[derive(Debug)]
pub(crate) enum Fault {
    /// `cargo metadata` printed something else than its JSON.
    Json(serde_json::Error),
    /// A member whose manifest is not `<dir>/Cargo.toml` below the workspace root.
    Manifest { package: Package, path: String },
    /// A changed path that is absolute, leaves the tree or is not text.
    Path { path: String },
    /// A line of `jj diff --summary` that is none of the forms.
    Summary { line: String },
    /// jj could not list the changes since the base.
    Diff { base: Base, source: Failure },
    /// The host call that ran `cargo metadata` failed.
    Host(Box<Error>),
    /// `cargo metadata` ended with a code other than 0; its output is in the log.
    Metadata { code: i32, log: PathBuf },
    /// `cargo metadata` was still running when the call ended.
    Running,
    /// Its log has no line of JSON.
    NoJson { log: PathBuf },
    /// The log cannot be read.
    Log { log: PathBuf, source: std::io::Error },
    /// Placement chose no host.
    NoPlace,
}

impl Fault {
    /// The exit it ends the command with: the host call's own, else the farm's.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Host(error) => error.exit(),
            _ => Exit::Farm,
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => write!(out, "`cargo metadata` did not print the JSON expected"),
            Self::Manifest { package, path } => {
                write!(out, "the manifest of `{package}` is {path}, not `<dir>/Cargo.toml` in the workspace")
            }
            Self::Path { path } => write!(out, "`{path}` is not a path inside the workspace"),
            Self::Summary { line } => write!(out, "`{line}` is not a line of `jj diff --summary`"),
            Self::Diff { base, .. } => write!(
                out,
                "`jj diff` from `{}` failed, so the changes are not known; name another base with `--base`",
                base.as_str()
            ),
            Self::Host(_) => write!(out, "the call that reads the crates of the workspace failed"),
            Self::Metadata { code, log } => write!(out, "`cargo metadata` ended with {code}; see {}", log.display()),
            Self::Running => write!(out, "`cargo metadata` was still running when the call ended; call again"),
            Self::NoJson { log } => write!(out, "`cargo metadata` printed no JSON; see {}", log.display()),
            Self::Log { log, .. } => write!(out, "cannot read {}", log.display()),
            Self::NoPlace => write!(out, "placement chose no host for `cargo metadata`"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Diff { source, .. } => Some(source),
            Self::Host(source) => Some(source.as_ref()),
            Self::Log { source, .. } => Some(source),
            Self::Manifest { .. }
            | Self::Path { .. }
            | Self::Summary { .. }
            | Self::Metadata { .. }
            | Self::Running
            | Self::NoJson { .. }
            | Self::NoPlace => None,
        }
    }
}
