//! `cargo xtask check`: the whole gate, in the order CI runs it. CI runs this one command, so the
//! list of steps lives here ([`Gate::steps`]) and nowhere else.

use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::component::{Components, Sites};
use crate::error::Error;
use crate::layout::Layout;
use crate::output::{Line, Report};
use crate::process::Process;

/// One step of the gate.
pub(crate) trait Step: fmt::Display {
    /// Runs it from the workspace at `root`, its lines and failures into `report`.
    fn run(&self, root: &Path, report: &mut Report);
}

/// A list of steps, run in order. Implemented for tuples of [`Step`]s.
pub(crate) trait Steps {
    /// Runs each, announced as it starts.
    fn run(&self, gate: &mut Gate);
}

tuples! { Steps: Step, fn run(&self, gate: &mut Gate) { |step| gate.step(step) } }

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

    /// Every step of the gate, in CI's order, each with `--locked`.
    fn steps() -> impl Steps {
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
                    "uses-ink-sdk",
                    "uses-ink-format-markdown-guest",
                    "uses-ink-theme-quiet-guest",
                    "uses-ink-host-wasmtime-hostile",
                ],
                flags: &[],
                target: Target::Browser,
            },
            // the plugins as components, which the tests and `Sites` load
            Components,
            Cargo { command: Command::Nextest, scope: Every, flags: &[], target: Target::Host },
            Cargo { command: Command::Test, scope: Every, flags: &[Flag::Doc], target: Target::Host },
            Cargo { command: Command::Deny, scope: (), flags: &[], target: Target::Host },
            // the pen's goldens hold on wasm too
            Cargo { command: Command::Test, scope: ["freehand"], flags: &[], target: Target::Wasi },
            // the browser pen as it ships
            Cargo {
                command: Command::Build,
                scope: ["freehand-web"],
                flags: &[Flag::Release],
                target: Target::Browser,
            },
            // every site fixture, natively and through the components
            Sites,
            Layout,
        )
    }

    /// Runs every step and reports every failure, then whether all passed.
    pub(crate) fn run(mut self) -> Report {
        Self::steps().run(&mut self);
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

/// A cargo command run as one step: a subcommand over a scope of crates, with flags, for a
/// target. `S` is the scope: [`Every`] crate, a list of packages, or `()` for none.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cargo<S> {
    pub(crate) command: Command,
    pub(crate) scope: S,
    pub(crate) flags: &'static [Flag],
    pub(crate) target: Target,
}

impl<S: Scope> Cargo<S> {
    /// The command line it runs, after `cargo`.
    fn invocation(&self) -> Invocation {
        let mut args = self.command.words().to_vec();
        args.extend(self.scope.args(self.command));
        args.extend(self.flags.iter().map(|flag| flag.arg()));
        args.extend(self.target.args());
        args.extend(self.command.tail());
        Invocation(args)
    }

    /// The tools outside the toolchain it needs.
    fn tools(&self) -> impl Iterator<Item = Tool> {
        self.command.tool().into_iter().chain(self.target.tool())
    }
}

impl<S: Scope> Step for Cargo<S> {
    fn run(&self, root: &Path, report: &mut Report) {
        let command = self.invocation();
        if let Some(tool) = self.tools().find(|tool| !tool.is_installed()) {
            return report.fail(Error::Tool { command, tool });
        }
        let mut process = std::process::Command::new(env!("CARGO"));
        process.args(&command.0).current_dir(root);
        report.take(Process::from(process).status().map_err(|source| Error::Step { command, source }));
    }
}

impl<S: Scope> fmt::Display for Cargo<S> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.invocation().fmt(out)
    }
}

/// A cargo command line, the words after `cargo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invocation(Vec<&'static str>);

impl fmt::Display for Invocation {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("cargo")?;
        self.0.iter().try_for_each(|arg| write!(out, " {arg}"))
    }
}

/// The crates a cargo command covers, as its arguments.
pub(crate) trait Scope {
    fn args(&self, command: Command) -> Vec<&'static str>;
}

/// Every crate of the workspace.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Every;

impl Scope for Every {
    fn args(&self, command: Command) -> Vec<&'static str> {
        vec![command.every()]
    }
}

/// The packages named.
impl<const N: usize> Scope for [&'static str; N] {
    fn args(&self, _: Command) -> Vec<&'static str> {
        self.iter().flat_map(|package| ["-p", package]).collect()
    }
}

/// No scope: the command covers what it covers.
impl Scope for () {
    fn args(&self, _: Command) -> Vec<&'static str> {
        Vec::new()
    }
}

/// A cargo subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    Fmt,
    Clippy,
    Nextest,
    Test,
    Deny,
    Build,
}

impl Command {
    /// The words that start it, `--locked` where it takes it.
    const fn words(self) -> &'static [&'static str] {
        match self {
            Self::Fmt => &["--locked", "fmt"],
            Self::Clippy => &["clippy", "--locked"],
            Self::Nextest => &["nextest", "run", "--locked"],
            Self::Test => &["test", "--locked"],
            Self::Deny => &["deny", "--locked", "check"],
            Self::Build => &["build", "--locked"],
        }
    }

    /// The words that end it: lints denied for clippy.
    const fn tail(self) -> &'static [&'static str] {
        match self {
            Self::Clippy => &["--", "-D", "warnings"],
            Self::Fmt | Self::Nextest | Self::Test | Self::Deny | Self::Build => &[],
        }
    }

    /// How it spells every crate of the workspace.
    const fn every(self) -> &'static str {
        match self {
            Self::Fmt => "--all",
            Self::Clippy | Self::Nextest | Self::Test | Self::Deny | Self::Build => "--workspace",
        }
    }

    /// The tool outside the toolchain it is.
    const fn tool(self) -> Option<Tool> {
        match self {
            Self::Nextest => Some(Tool::Nextest),
            Self::Deny => Some(Tool::Deny),
            Self::Fmt | Self::Clippy | Self::Test | Self::Build => None,
        }
    }
}

/// A flag a step passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flag {
    /// Check formatting without writing.
    Check,
    /// Tests, examples and benches too.
    AllTargets,
    /// Doc tests only.
    Doc,
    Release,
    /// The `component` profile the plugins ship with (decision 0035).
    Component,
}

impl Flag {
    const fn arg(self) -> &'static str {
        match self {
            Self::Check => "--check",
            Self::AllTargets => "--all-targets",
            Self::Doc => "--doc",
            Self::Release => "--release",
            Self::Component => "--profile=component",
        }
    }
}

/// What a step builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// This machine.
    Host,
    /// The browser: `wasm32-unknown-unknown`.
    Browser,
    /// WASI preview 2, run by wasmtime: `wasm32-wasip2`.
    Wasi,
}

impl Target {
    const fn args(self) -> &'static [&'static str] {
        match self {
            Self::Host => &[],
            Self::Browser => &["--target", "wasm32-unknown-unknown"],
            Self::Wasi => &["--target", "wasm32-wasip2"],
        }
    }

    /// The tool outside the toolchain that runs what it builds.
    const fn tool(self) -> Option<Tool> {
        match self {
            Self::Wasi => Some(Tool::Wasmtime),
            Self::Host | Self::Browser => None,
        }
    }
}

/// A tool a step needs that the toolchain does not bring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Nextest,
    Deny,
    /// The runner for wasm32-wasip2 tests (`.cargo/config.toml`).
    Wasmtime,
}

impl Tool {
    /// The executable looked for on `PATH`.
    pub(crate) const fn binary(self) -> &'static str {
        match self {
            Self::Nextest => "cargo-nextest",
            Self::Deny => "cargo-deny",
            Self::Wasmtime => "wasmtime",
        }
    }

    /// The command that installs it.
    pub(crate) const fn install(self) -> &'static str {
        match self {
            Self::Nextest => "cargo install --locked cargo-nextest",
            Self::Deny => "cargo install --locked cargo-deny",
            Self::Wasmtime => "cargo install --locked wasmtime-cli",
        }
    }

    fn is_installed(self) -> bool {
        std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(self.binary()).is_file()))
    }
}
