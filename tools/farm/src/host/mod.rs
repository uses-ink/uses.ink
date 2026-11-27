//! A host as the config describes it, and as a probe finds it.

mod dir;
#[cfg(test)]
pub(crate) mod fake;
mod fleet;
mod member;
mod queue;
mod snapshot;
pub(crate) mod transport;

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;

pub(crate) use dir::{DirFault, HostDir};
pub(crate) use fleet::{Fleet, Probed};
pub(crate) use member::{Member, Start};
pub(crate) use snapshot::{JobReadFault, ReadFault, Snapshot};

use crate::cli::name::{Destination, HostName};
use crate::place::wake::Wake;

/// A CPU architecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Arch {
    X86,
    Aarch64,
    /// Any other, as `uname -m` says it.
    Other(String),
}

impl Arch {
    /// The architecture the repo's goldens are blessed on (docs/conventions.md, Tests).
    pub(crate) const REFERENCE: Self = Self::X86;
}

impl FromStr for Arch {
    type Err = std::convert::Infallible;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        Ok(match given {
            "x86_64" | "amd64" => Self::X86,
            "aarch64" | "arm64" => Self::Aarch64,
            other => Self::Other(other.to_owned()),
        })
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86 => out.write_str("x86_64"),
            Self::Aarch64 => out.write_str("aarch64"),
            Self::Other(other) => out.write_str(other),
        }
    }
}

/// How a host is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Via {
    /// This machine, through `sh`, in the workspace itself.
    Local,
    /// Another machine, through `ssh` and `rsync`.
    Ssh(Destination),
}

impl FromStr for Via {
    type Err = crate::cli::name::Fault;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        match given {
            "local" => Ok(Self::Local),
            other => other.parse().map(Self::Ssh),
        }
    }
}

/// A host from the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Host {
    pub(crate) name: HostName,
    pub(crate) via: Via,
    /// Slots by config; `None` is cores divided by six, at least one.
    pub(crate) slots: Option<u32>,
    /// Arch by config; `None` is what the probe finds.
    pub(crate) arch: Option<Arch>,
    /// Whether goldens may run here by config; `None` is whether the arch is the reference.
    pub(crate) golden: Option<bool>,
    pub(crate) mold: bool,
    pub(crate) dir: HostDir,
    /// How to start the host when it stops itself, if it does.
    pub(crate) wake: Option<Wake>,
}

impl Host {
    /// This machine, with every default.
    pub(crate) fn local() -> Self {
        Self {
            name: HostName::local(),
            via: Via::Local,
            slots: None,
            arch: None,
            golden: None,
            mold: false,
            dir: HostDir::standard(),
            wake: None,
        }
    }
}

/// A load average, in hundredths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Load(pub(crate) u32);

impl FromStr for Load {
    type Err = ParseIntError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
        let mut hundredths: String = frac.chars().take(2).collect();
        while hundredths.len() < 2 {
            hundredths.push('0');
        }
        let whole: u32 = whole.parse()?;
        Ok(Self(whole.saturating_mul(100).saturating_add(hundredths.parse()?)))
    }
}

impl fmt::Display for Load {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}
