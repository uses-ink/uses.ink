//! The cases a Worker run can be asked for.

use std::fmt;
use std::str::FromStr;

use super::fault::Fault;
use super::migrate::Migrated;
use super::node::Packages;
use super::node::Shell;
use super::probe::Probe;
use super::serve::Served;
use crate::error::Error;
use crate::report::{Line, Report};

/// What `cargo xtask worker` can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Case {
    /// What the host has: node, npm, targets, the network.
    Probe,
    /// Resolves the npm packages and prints the lockfiles, to be committed.
    Lock,
    /// Spike (a): the API crates and the passkey verifier, built, measured and run in workerd.
    Api,
    /// Spikes (b) and (c): D1 values and R2 streaming, in workerd.
    Store,
    /// The row port's conformance suite, run through the D1 adapter in workerd.
    Db,
    /// H1b: the generated migrations applied by wrangler, compared with SQLite's schema.
    Migrate,
}

impl Case {
    /// Every case, in the order they are listed.
    pub(crate) const ALL: [Self; 6] = [Self::Probe, Self::Lock, Self::Api, Self::Store, Self::Db, Self::Migrate];

    /// The cases the gate's stage `worker` runs: tests, not spikes.
    pub(crate) const GATE: [Self; 2] = [Self::Db, Self::Migrate];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Lock => "lock",
            Self::Api => "api",
            Self::Store => "store",
            Self::Db => "db",
            Self::Migrate => "migrate",
        }
    }

    /// The folder of its Worker crate, relative to the root.
    pub(super) const fn dir(self) -> &'static str {
        match self {
            Self::Probe | Self::Lock => "spikes/hosted",
            Self::Migrate => "spikes/hosted/workers/migrate",
            Self::Api => "spikes/hosted/workers/api",
            Self::Store => "spikes/hosted/workers/store",
            Self::Db => "spikes/hosted/workers/rig",
        }
    }

    /// The port its `wrangler dev` listens on: one each, so two runs on one host do not collide.
    pub(super) const fn port(self) -> u16 {
        match self {
            Self::Probe | Self::Lock | Self::Migrate => 0,
            Self::Api => 8731,
            Self::Store => 8732,
            Self::Db => 8733,
        }
    }

    /// Runs it, its lines into `report`.
    pub(super) fn run(self, shell: &Shell, report: &mut Report) -> Result<(), Fault> {
        match self {
            Self::Probe => {
                Probe { shell }.run(report);
                Ok(())
            }
            Self::Lock => Self::lock(shell, report),
            Self::Migrate => Migrated { shell }.run(report),
            Self::Api | Self::Store | Self::Db => Served { case: self, shell }.run(report),
        }
    }

    /// Prints the lockfiles a run needs committed: npm's, resolved here, and the cargo ones as
    /// cargo wrote them on this host.
    fn lock(shell: &Shell, report: &mut Report) -> Result<(), Fault> {
        for manifest in ["Cargo.toml", "spikes/hosted/Cargo.toml"] {
            let mut fetch = shell.command(env!("CARGO"), shell.root());
            fetch.args(["fetch", "--manifest-path", manifest]);
            Shell::run(fetch)?;
        }
        let npm = Packages { shell };
        let mut files = vec![("spikes/hosted/package-lock.json", npm.resolve()?)];
        for path in ["Cargo.lock", "spikes/hosted/Cargo.lock"] {
            let text = Shell::read(&shell.root().join(path)).map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
            files.push((path, text.unwrap_or_default()));
        }
        for (path, text) in files {
            report.line(Line::Worker { case: Self::Lock, text: format!("=====BEGIN {path}=====") });
            for line in text.lines() {
                report.line(Line::Worker { case: Self::Lock, text: line.to_owned() });
            }
            report.line(Line::Worker { case: Self::Lock, text: format!("=====END {path}=====") });
        }
        Ok(())
    }
}

impl fmt::Display for Case {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for Case {
    type Err = Error;

    fn from_str(given: &str) -> Result<Self, Error> {
        Self::ALL.into_iter().find(|case| case.name() == given).ok_or_else(|| Error::Case { given: given.to_owned() })
    }
}
