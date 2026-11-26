//! `cargo xtask check`: the whole gate, in the order CI runs it. CI runs this one command, so the
//! list of checks lives here and nowhere else.

use std::fmt;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use crate::error::Error;
use crate::output::Line;
use crate::process::Process;

/// One step of the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Fmt,
    Clippy,
    /// Clippy on the crates that are built for the browser and for plugins.
    ClippyWasm,
    Nextest,
    Doc,
    Deny,
    /// The pen's tests on wasm32-wasip2: its goldens hold on wasm too.
    WasmTest,
    /// The browser pen as it ships.
    WasmBuild,
    /// `cargo xtask layout`, in this process.
    Layout,
}

impl Step {
    /// Every step, in the order they run.
    pub(crate) const ALL: [Self; 9] = [
        Self::Fmt,
        Self::Clippy,
        Self::ClippyWasm,
        Self::Nextest,
        Self::Doc,
        Self::Deny,
        Self::WasmTest,
        Self::WasmBuild,
        Self::Layout,
    ];

    /// The arguments to cargo that run it, `--locked` in each; `None` for a step run in this
    /// process.
    const fn cargo(self) -> Option<&'static [&'static str]> {
        Some(match self {
            Self::Fmt => &["--locked", "fmt", "--all", "--check"],
            Self::Clippy => &["clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"],
            Self::ClippyWasm => &[
                "clippy",
                "--locked",
                "-p",
                "freehand",
                "-p",
                "freehand-web",
                "-p",
                "uses-ink-model",
                "-p",
                "uses-ink-format-markdown",
                "--target",
                "wasm32-unknown-unknown",
                "--",
                "-D",
                "warnings",
            ],
            Self::Nextest => &["nextest", "run", "--locked", "--workspace"],
            Self::Doc => &["test", "--locked", "--workspace", "--doc"],
            Self::Deny => &["deny", "--locked", "check"],
            Self::WasmTest => &["test", "--locked", "-p", "freehand", "--target", "wasm32-wasip2"],
            Self::WasmBuild => {
                &["build", "--locked", "-p", "freehand-web", "--release", "--target", "wasm32-unknown-unknown"]
            }
            Self::Layout => return None,
        })
    }

    /// The tool outside the toolchain it needs.
    const fn tool(self) -> Option<Tool> {
        match self {
            Self::Nextest => Some(Tool::Nextest),
            Self::Deny => Some(Tool::Deny),
            Self::WasmTest => Some(Tool::Wasmtime),
            Self::Fmt | Self::Clippy | Self::ClippyWasm | Self::Doc | Self::WasmBuild | Self::Layout => None,
        }
    }

    fn run(self, root: &Path) -> Result<(), Error> {
        if let Some(tool) = self.tool().filter(|tool| !tool.is_installed()) {
            return Err(Error::Tool { step: self, tool });
        }
        let Some(args) = self.cargo() else { return crate::layout(root).map(drop) };
        let mut command = Command::new(env!("CARGO"));
        command.args(args).current_dir(root);
        Process::from(command).status().map_err(|source| Error::Step { step: self, source })
    }
}

impl fmt::Display for Step {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Fmt => "fmt",
            Self::Clippy => "clippy",
            Self::ClippyWasm => "clippy on wasm32",
            Self::Nextest => "nextest",
            Self::Doc => "doc tests",
            Self::Deny => "cargo deny",
            Self::WasmTest => "freehand tests on wasm32-wasip2",
            Self::WasmBuild => "freehand-web release build",
            Self::Layout => "layout and generated files",
        })
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
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path).any(|dir| dir.join(self.binary()).is_file())
    }
}

/// Runs every step, each announced on stderr as it starts, and fails with every failed step.
pub(crate) fn run(root: &Path) -> Result<Vec<Line>, Error> {
    let mut err = std::io::stderr();
    let mut failed = Vec::new();
    for step in Step::ALL {
        writeln!(err, "check: {step}").map_err(Error::Progress)?;
        if let Err(error) = step.run(root) {
            failed.push(error);
        }
    }
    if failed.is_empty() { Ok(vec![Line::Passed]) } else { Err(Error::Each(failed)) }
}
