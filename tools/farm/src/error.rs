//! Everything the farm can fail with. `Display` says what failed, `source()` the cause; `main`
//! prints the chain. [`Error::exit`] maps each to its [`Exit`].

use std::fmt;
use std::path::PathBuf;

use crate::cli::Usage;
use crate::config;
use crate::event;
use crate::exit::Exit;
use crate::host::ReadFault;
use crate::jobs;
use crate::name::{Fault, HostName, JobId};
use crate::place::Need;
use crate::process::Failure;
use crate::workspace;

/// One failure of the farm: it could not do what was asked, or would not.
#[derive(Debug)]
pub(crate) enum Error {
    Usage(Usage),
    /// The config file.
    Config {
        path: PathBuf,
        fault: config::Fault,
    },
    /// The repo's job file.
    Jobs {
        path: PathBuf,
        fault: jobs::Fault,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// No jj workspace around the current directory.
    NoRepo {
        start: PathBuf,
    },
    /// A path that is not UTF-8, so it cannot be an argument.
    Utf8 {
        path: PathBuf,
    },
    /// The workspace's directory names are not usable as path segments.
    Name {
        path: PathBuf,
        source: Fault,
    },
    /// A program or `rsync` on a host that failed.
    Call {
        host: HostName,
        what: &'static str,
        source: Failure,
    },
    /// A probe that printed what a probe does not.
    Probe {
        host: HostName,
        source: ReadFault,
    },
    /// A program that said what no program says.
    Said {
        host: HostName,
        source: event::Fault,
    },
    /// A program that did not say what it must.
    Silent {
        host: HostName,
        what: &'static str,
    },
    /// No host can run the job.
    NoHost {
        need: Need,
        up: Vec<HostName>,
    },
    /// A golden job pinned to a host that is not golden.
    NotGolden {
        host: HostName,
    },
    /// The connection ended and the job was stopped with it.
    Lost {
        job: JobId,
    },
    /// The host could not tell that the job's body started, so it may be running unobserved.
    NoBody {
        job: JobId,
    },
    /// A job the host has no record of.
    NoJob {
        job: JobId,
    },
    /// A thread running a job panicked.
    Crashed {
        label: String,
    },
    /// A changed path that cannot be trusted.
    Changes {
        host: HostName,
        source: crate::changes::Fault,
    },
    /// Files edited here after the sync started, which bringing the host's copy back would overwrite.
    Edited {
        host: HostName,
        paths: Vec<String>,
    },
    Workspace(workspace::Fault),
}

impl Error {
    /// The exit it ends the command with.
    pub(crate) fn exit(&self) -> Exit {
        match self {
            Self::Usage(_) => Exit::Usage,
            Self::Jobs { fault, .. } if fault.is_usage() => Exit::Usage,
            Self::NotGolden { .. } | Self::Edited { .. } => Exit::Refused,
            Self::Workspace(fault) if fault.is_refusal() => Exit::Refused,
            Self::Config { .. }
            | Self::Jobs { .. }
            | Self::Io { .. }
            | Self::NoRepo { .. }
            | Self::Utf8 { .. }
            | Self::Name { .. }
            | Self::Call { .. }
            | Self::Probe { .. }
            | Self::Said { .. }
            | Self::Silent { .. }
            | Self::NoHost { .. }
            | Self::Lost { .. }
            | Self::NoBody { .. }
            | Self::NoJob { .. }
            | Self::Crashed { .. }
            | Self::Changes { .. }
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
            Self::Jobs { path, fault } => write!(out, "the job file {} {fault}", path.display()),
            Self::Io { path, .. } => write!(out, "cannot read {}", path.display()),
            Self::NoRepo { start } => write!(out, "{} is not inside a jj workspace", start.display()),
            Self::Utf8 { path } => write!(out, "{} is not UTF-8", path.display()),
            Self::Name { path, .. } => write!(out, "{} cannot name a farm directory", path.display()),
            Self::Call { host, what, .. } => write!(out, "{what} on `{host}` failed"),
            Self::Probe { host, .. } => write!(out, "`{host}` answered its probe with something unreadable"),
            Self::Said { host, .. } => write!(out, "`{host}` said something unreadable"),
            Self::Silent { host, what } => write!(out, "`{host}` did not say {what}"),
            Self::NoHost { need: Need::Golden, up } => {
                write!(
                    out,
                    "no golden host to run on; the hosts up are {}; a golden host is x86_64 or `golden = true` in the config",
                    Listed(up)
                )
            }
            Self::NoHost { need, up } => write!(out, "no {need} host to run on; the hosts up are {}", Listed(up)),
            Self::NotGolden { host } => {
                write!(out, "`{host}` is not a golden host, and this job's output depends on the architecture")
            }
            Self::Lost { job } => {
                write!(out, "the connection for job `{job}` ended before it did, and the host stopped the job")
            }
            Self::NoBody { job } => write!(
                out,
                "the host did not report the start of job `{job}` in time; it may be running, see `farm wait {job}` and `farm log {job}`"
            ),
            Self::NoJob { job } => {
                write!(out, "the host has no record of job `{job}`; it may be older than seven days")
            }
            Self::Crashed { label } => write!(out, "the thread running `{label}` crashed"),
            Self::Changes { host, .. } => write!(out, "`{host}` listed a changed file that is not safe to copy"),
            Self::Edited { host, paths } => write!(
                out,
                "{} edited here after the sync to `{host}` started; nothing was brought back, so run the verb again once you are done editing",
                Listed(paths)
            ),
            Self::Workspace(fault) => write!(out, "{fault}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage(source) => source.source(),
            Self::Config { fault, .. } => std::error::Error::source(fault),
            Self::Jobs { fault, .. } => std::error::Error::source(fault),
            Self::Io { source, .. } => Some(source),
            Self::Name { source, .. } => Some(source),
            Self::Call { source, .. } => Some(source),
            Self::Probe { source, .. } => Some(source),
            Self::Said { source, .. } => Some(source),
            Self::Workspace(fault) => fault.source(),
            Self::Changes { source, .. } => Some(source),
            Self::NoRepo { .. }
            | Self::Utf8 { .. }
            | Self::Silent { .. }
            | Self::NoHost { .. }
            | Self::NotGolden { .. }
            | Self::Edited { .. }
            | Self::Lost { .. }
            | Self::NoBody { .. }
            | Self::NoJob { .. }
            | Self::Crashed { .. } => None,
        }
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

/// Items in backticks, as a sentence lists them: `a`, `b` and `c`.
pub(crate) struct Listed<'a, T>(pub(crate) &'a [T]);

impl<T: fmt::Display> fmt::Display for Listed<'_, T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last = self.0.len().saturating_sub(1);
        self.0.iter().enumerate().try_for_each(|(i, item)| {
            let sep = match i {
                0 => "",
                i if i == last => " and ",
                _ => ", ",
            };
            write!(out, "{sep}`{item}`")
        })
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
            (Error::Edited { host: host(), paths: vec!["a.rs".to_owned()] }, Exit::Refused),
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
