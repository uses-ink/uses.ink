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
    /// The blob port's conformance suite, run through the R2 adapter in workerd.
    Blob,
    /// The queue port's conformance suite, run through the D1 rows and a Queues wake-up in workerd.
    Queue,
    /// The secret port's conformance suite, run through a Workers secret binding in workerd.
    Secret,
    /// Every conformance suite of the rig, in one Worker: what the gate runs.
    Rig,
    /// H1b: the generated migrations applied by wrangler, compared with SQLite's schema.
    Migrate,
}

impl Case {
    /// Every case, in the order they are listed.
    pub(crate) const ALL: [Self; 10] = [
        Self::Probe,
        Self::Lock,
        Self::Api,
        Self::Store,
        Self::Db,
        Self::Blob,
        Self::Queue,
        Self::Secret,
        Self::Rig,
        Self::Migrate,
    ];

    /// The cases the gate's stage `worker` runs: tests, not spikes.
    pub(crate) const GATE: [Self; 2] = [Self::Rig, Self::Migrate];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Lock => "lock",
            Self::Api => "api",
            Self::Store => "store",
            Self::Db => "db",
            Self::Blob => "blob",
            Self::Queue => "queue",
            Self::Secret => "secret",
            Self::Rig => "rig",
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
            Self::Db | Self::Blob | Self::Queue | Self::Secret | Self::Rig => "spikes/hosted/workers/rig",
        }
    }

    /// The port its `wrangler dev` listens on: one each, so two runs on one host do not collide.
    pub(super) const fn port(self) -> u16 {
        match self {
            Self::Probe | Self::Lock | Self::Migrate => 0,
            Self::Api => 8731,
            Self::Store => 8732,
            Self::Db | Self::Blob | Self::Queue | Self::Secret | Self::Rig => 8733,
        }
    }

    /// The suite of the rig this case runs alone, or none for every suite.
    pub(super) const fn suite(self) -> Option<&'static str> {
        match self {
            Self::Db => Some("db"),
            Self::Blob => Some("blob"),
            Self::Queue => Some("queue"),
            Self::Secret => Some("secret"),
            Self::Probe | Self::Lock | Self::Api | Self::Store | Self::Rig | Self::Migrate => None,
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
            Self::Api | Self::Store | Self::Db | Self::Blob | Self::Queue | Self::Secret | Self::Rig => {
                Served { case: self, shell }.run(report)
            }
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
