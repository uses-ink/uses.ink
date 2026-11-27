//! Everything the farm can fail with. `Display` says what failed, `source()` the cause; `main`
//! prints the chain. [`Error::exit`] maps each to its [`Exit`].

use std::fmt;
use std::path::PathBuf;

use crate::error::Listed;
use crate::farm::cli::Usage;
use crate::farm::config;
use crate::farm::event;
use crate::farm::exit::Exit;
use crate::farm::host::ReadFault;
use crate::farm::name::{Fault, HostName, JobId};
use crate::farm::place::Need;
use crate::farm::workspace;
use crate::process::Failure;

/// One failure of the farm: it could not do what was asked, or would not.
#[derive(Debug)]
pub(crate) enum Error {
    Usage(Usage),
    /// The config file.
    Config { path: PathBuf, fault: config::Fault },
    Io { path: PathBuf, source: std::io::Error },
    /// A path that is not UTF-8, so it cannot be an argument.
    Utf8 { path: PathBuf },
    /// The workspace's directory names are not usable as path segments.
    Name { path: PathBuf, source: Fault },
    /// A program or `rsync` on a host that failed.
    Call { host: HostName, what: &'static str, source: Failure },
    /// A probe that printed what a probe does not.
    Probe { host: HostName, source: ReadFault },
    /// A program that said what no program says.
    Said { host: HostName, source: event::Fault },
    /// A program that did not say what it must.
    Silent { host: HostName, what: &'static str },
    /// No host can run the job.
    NoHost { need: Need, up: Vec<HostName> },
    /// A golden job pinned to a host that is not golden.
    NotGolden { host: HostName },
    /// The connection ended and the job was stopped with it.
    Lost { job: JobId },
    /// A job the host has no record of.
    NoJob { job: JobId },
    /// A thread running a job panicked.
    Crashed { label: String },
    /// A changed snapshot path that cannot be trusted.
    Snap { host: HostName, source: crate::farm::snap::Fault },
    Workspace(workspace::Fault),
}

impl Error {
    /// The exit it ends the command with.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Usage(_) => Exit::Usage,
            Self::NotGolden { .. } => Exit::Refused,
            Self::Workspace(fault) if fault.is_refusal() => Exit::Refused,
            Self::Config { .. }
            | Self::Io { .. }
            | Self::Utf8 { .. }
            | Self::Name { .. }
            | Self::Call { .. }
            | Self::Probe { .. }
            | Self::Said { .. }
            | Self::Silent { .. }
            | Self::NoHost { .. }
            | Self::Lost { .. }
            | Self::NoJob { .. }
            | Self::Crashed { .. }
            | Self::Snap { .. }
            | Self::Workspace(_) => Exit::Farm,
        }
    }
}

impl From<Usage> for Error {
    fn from(usage: Usage) -> Self {
        Self::Usage(usage)
    }
}

impl From<workspace::Fault> for Error {
    fn from(fault: workspace::Fault) -> Self {
        Self::Workspace(fault)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(usage) => write!(out, "{usage}"),
            Self::Config { path, fault } => write!(out, "the farm config {} {fault}", path.display()),
            Self::Io { path, .. } => write!(out, "cannot read {}", path.display()),
            Self::Utf8 { path } => write!(out, "{} is not UTF-8", path.display()),
            Self::Name { path, .. } => write!(out, "{} cannot name a farm directory", path.display()),
            Self::Call { host, what, .. } => write!(out, "{what} on `{host}` failed"),
            Self::Probe { host, .. } => write!(out, "`{host}` answered its probe with something unreadable"),
            Self::Said { host, .. } => write!(out, "`{host}` said something unreadable"),
            Self::Silent { host, what } => write!(out, "`{host}` did not say {what}"),
            Self::NoHost { need: Need::Golden, up } => {
                write!(out, "no golden host to run on; the hosts up are {}; a golden host is x86_64 or `golden = true` in the config", Listed(up))
            }
            Self::NoHost { need, up } => write!(out, "no {need} host to run on; the hosts up are {}", Listed(up)),
            Self::NotGolden { host } => write!(out, "`{host}` is not a golden host, and this job's output depends on the architecture"),
            Self::Lost { job } => write!(out, "the connection for job `{job}` ended before it did, and the host stopped the job"),
            Self::NoJob { job } => write!(out, "the host has no record of job `{job}`; it may be older than seven days"),
            Self::Crashed { label } => write!(out, "the thread running `{label}` crashed"),
            Self::Snap { host, .. } => write!(out, "`{host}` listed a changed snapshot that is not safe to copy"),
            Self::Workspace(fault) => write!(out, "{fault}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage(source) => source.source(),
            Self::Config { fault, .. } => std::error::Error::source(fault),
            Self::Io { source, .. } => Some(source),
            Self::Name { source, .. } => Some(source),
            Self::Call { source, .. } => Some(source),
            Self::Probe { source, .. } => Some(source),
            Self::Said { source, .. } => Some(source),
            Self::Workspace(fault) => fault.source(),
            Self::Snap { source, .. } => Some(source),
            Self::Utf8 { .. }
            | Self::Silent { .. }
            | Self::NoHost { .. }
            | Self::NotGolden { .. }
            | Self::Lost { .. }
            | Self::NoJob { .. }
            | Self::Crashed { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostName {
        "petal".parse().unwrap()
    }

    #[test]
    fn every_failure_maps_to_its_exit() {
        let table = [
            (Error::Usage(Usage::NoVerb), Exit::Usage),
            (Error::NotGolden { host: host() }, Exit::Refused),
            (Error::NoHost { need: Need::Golden, up: vec![] }, Exit::Farm),
            (Error::Silent { host: host(), what: "x" }, Exit::Farm),
            (Error::Workspace(workspace::Fault::Dirty { name: "a".parse().unwrap() }), Exit::Refused),
            (Error::Workspace(workspace::Fault::Missing { path: PathBuf::from("/x") }), Exit::Farm),
        ];
        for (error, exit) in table {
            assert_eq!(error.exit(), exit, "{error}");
        }
    }

    #[test]
    fn no_golden_host_says_what_golden_is() {
        let text = Error::NoHost { need: Need::Golden, up: vec![host()] }.to_string();
        assert!(text.contains("x86_64") && text.contains("`petal`"), "{text}");
    }
}

/// An error and its causes as the person reads them: `error: ...`, then a `caused by: ...` line each.
#[derive(Debug)]
pub(crate) struct Chain<'a>(pub(crate) &'a Error);

impl fmt::Display for Chain<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "error: {}", self.0)?;
        let mut cause = std::error::Error::source(self.0);
        while let Some(error) = cause {
            write!(out, "\n  caused by: {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}
