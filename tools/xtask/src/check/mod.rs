//! `cargo xtask check [stage]`: the whole gate, or one [`Stage`] of it, which CI runs as a job of
//! its own. The list of steps lives here ([`Gate`]) and nowhere else.

mod cargo;
mod steps;

use std::fmt;
use std::io::Write as _;
use std::path::PathBuf;
use std::str::FromStr;

use crate::build::component::Components;
use crate::build::site::Sites;
use crate::docs::Prose;
use crate::error::Error;
use crate::layout::Layout;
use crate::report::{Line, Report};
use crate::worker::{Case, Worker};

pub(crate) use cargo::{Cargo, Command, Every, Flag, Invocation, Target, Tool};
pub(crate) use steps::{Step, Steps};

/// The gate as it runs: every step's lines and failures, gathered.
#[derive(Debug)]
pub(crate) struct Gate {
    root: PathBuf,
    report: Report,
}

impl Gate {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), report: Report::default() }
    }

    /// The checks that read the source: formatting, lints, licences and the layout rules.
    fn lint() -> impl Steps {
        (
            Cargo { command: Command::Fmt, scope: Every, flags: &[Flag::Check], target: Target::Host },
            Cargo { command: Command::Clippy, scope: Every, flags: &[Flag::AllTargets], target: Target::Host },
            // the crates built for the browser and for plugins
            Cargo {
                command: Command::Clippy,
                scope: [
                    "freehand",
                    "freehand-web",
                    "uses-ink-model",
                    "uses-ink-format-markdown",
                    "uses-ink-format-obsidian",
                    "uses-ink-sdk",
                    "uses-ink-format-markdown-guest",
                    "uses-ink-format-obsidian-guest",
                    "uses-ink-theme-quiet-guest",
                    "uses-ink-source-git",
                    "uses-ink-source-git-guest",
                    "uses-ink-host-wasmtime-hostile",
                    "uses-ink-db-d1",
                    "uses-ink-blob-r2",
                    "uses-ink-edge-api",
                ],
                flags: &[],
                target: Target::Browser,
            },
            Cargo { command: Command::Deny, scope: (), flags: &[], target: Target::Host },
            Layout,
        )
    }

    /// The tests: the plugins as components, which the tests load, then every test.
    fn test() -> impl Steps {
        (Components, Cargo { command: Command::Nextest, scope: Every, flags: &[], target: Target::Host })
    }

    /// The docs checks, then the doc tests, each compiled and linked on its own, which makes them the slowest step.
    fn docs() -> impl Steps {
        (Prose, Cargo { command: Command::Test, scope: Every, flags: &[Flag::Doc], target: Target::Host })
    }

    /// The crates that also run on wasm.
    fn wasm() -> impl Steps {
        (
            // the pen's goldens hold on wasm too
            Cargo { command: Command::Test, scope: ["freehand"], flags: &[], target: Target::Wasi },
            // the browser pen as it ships
            Cargo {
                command: Command::Build,
                scope: ["freehand-web"],
                flags: &[Flag::Release],
                target: Target::Browser,
            },
        )
    }

    /// Every site fixture, built natively and through the components.
    fn sites() -> impl Steps {
        (Components, Sites)
    }

    /// The Worker tests: what runs in `workerd` with no account.
    fn worker() -> impl Steps {
        (Worker(Case::GATE[0]), Worker(Case::GATE[1]))
    }

    /// Runs the stage, or every stage in order, and reports every failure, then whether all passed.
    pub(crate) fn run(mut self, stage: Option<Stage>) -> Report {
        match stage {
            Some(stage) => stage.run(&mut self),
            None => Stage::ALL.into_iter().for_each(|stage| stage.run(&mut self)),
        }
        if self.report.failures().is_empty() {
            self.report.line(Line::Passed);
        }
        self.report
    }

    /// Runs one step, announced on stderr as it starts.
    fn step(&mut self, step: &impl Step) {
        if let Err(error) = writeln!(std::io::stderr(), "check: {step}") {
            self.report.fail(Error::Progress(error));
        }
        step.run(&self.root, &mut self.report);
    }
}

/// A part of the gate that CI runs as a job of its own: `cargo xtask check <stage>`. Without a
/// stage the gate runs all of them in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Lint,
    Test,
    Docs,
    Wasm,
    Sites,
    Worker,
}

impl Stage {
    /// Every stage, in the order the whole gate runs them.
    pub(crate) const ALL: [Self; 6] = [Self::Lint, Self::Test, Self::Docs, Self::Wasm, Self::Sites, Self::Worker];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Lint => "lint",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::Wasm => "wasm",
            Self::Sites => "sites",
            Self::Worker => "worker",
        }
    }

    /// Runs its steps in `gate`.
    fn run(self, gate: &mut Gate) {
        match self {
            Self::Lint => Gate::lint().run(gate),
            Self::Test => Gate::test().run(gate),
            Self::Docs => Gate::docs().run(gate),
            Self::Wasm => Gate::wasm().run(gate),
            Self::Sites => Gate::sites().run(gate),
            Self::Worker => Gate::worker().run(gate),
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for Stage {
    type Err = Error;

    fn from_str(given: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|stage| stage.name() == given)
            .ok_or_else(|| Error::Stage { given: given.to_owned() })
    }
}
