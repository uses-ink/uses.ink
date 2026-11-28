//! How a call reaches a host. [`Transport`] is the one seam: [`Local`] is this machine through
//! `sh`, [`Ssh`] is another through the system `ssh` and `rsync`, and the logic above them is
//! tested with a fake. Every process is started from an argument vector; the one shell line,
//! the remote command of `ssh`, is made of [`Quoted`](crate::job::quote::Quoted) words.

mod cancel;
mod exec;
mod local;
pub(crate) mod process;
pub(crate) mod program;
mod ssh;

use std::path::Path;
use std::process::ExitStatus;

pub(crate) use cancel::Cancel;
pub(crate) use exec::Exec;
pub(crate) use local::Local;
pub(crate) use ssh::Ssh;

use crate::host::transport::process::Failure;
use crate::host::transport::program::Call;

/// What a call left: how it ended and what it said on stderr.
#[derive(Debug)]
pub(crate) struct Done {
    pub(crate) status: ExitStatus,
    pub(crate) stderr: String,
}

impl Done {
    /// The call's result, or why it failed.
    pub(crate) fn ok(self) -> Result<Self, Failure> {
        if self.status.success() {
            Ok(self)
        } else {
            Err(Failure::Exit { status: self.status, stderr: self.stderr.into_bytes() })
        }
    }
}

/// A workspace to ship to a host.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Push<'a> {
    /// The workspace here.
    pub(crate) root: &'a Path,
    /// Its directory on the host, under the host's `dir`.
    pub(crate) tree: &'a str,
    /// The lock that keeps two syncs of it from overlapping.
    pub(crate) lock: &'a str,
    /// Names the repo keeps out of the sync, beside the built-in ones.
    pub(crate) exclude: &'a [crate::cli::name::Excluded],
}

/// Files to bring back from a host's tree into the workspace.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pull<'a> {
    pub(crate) root: &'a Path,
    pub(crate) tree: &'a str,
    /// Paths relative to both, one per line.
    pub(crate) files: &'a str,
}

/// A way to reach a host.
pub(crate) trait Transport {
    /// Runs a call, handing each line it prints to `lines`.
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure>;

    /// Makes the host's tree equal to the workspace. Nothing for a host that runs in place.
    fn push(&self, push: &Push<'_>) -> Result<(), Failure>;

    /// Copies the listed files from the host's tree into the workspace.
    fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure>;

    /// Whether the host runs in the workspace itself, with no copy.
    fn is_local(&self) -> bool;
}

/// Either way of reaching a host: what a [`Fleet`](crate::host::Fleet) is made of.
#[derive(Debug, Clone)]
pub(crate) enum Link {
    Local(Local),
    Ssh(Ssh),
}

impl Transport for Link {
    fn call(&self, call: &Call, lines: &mut impl FnMut(&str)) -> Result<Done, Failure> {
        match self {
            Self::Local(local) => local.call(call, lines),
            Self::Ssh(ssh) => ssh.call(call, lines),
        }
    }

    fn push(&self, push: &Push<'_>) -> Result<(), Failure> {
        match self {
            Self::Local(local) => local.push(push),
            Self::Ssh(ssh) => ssh.push(push),
        }
    }

    fn pull(&self, pull: &Pull<'_>) -> Result<(), Failure> {
        match self {
            Self::Local(local) => local.pull(pull),
            Self::Ssh(ssh) => ssh.pull(pull),
        }
    }

    fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }
}
