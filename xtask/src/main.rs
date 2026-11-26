//! Repo tasks, run as `cargo xtask <task>`:
//!
//! - `token`: write `token/src/generated.rs` and `token/token.css` from `token/token.toml`.
//! - `diag`: write `model/src/generated.rs` and `diag/src/generated.rs` from `diag/catalog.toml`.
//! - `brand`: draw the logo, mark and favicon into `brand/` from `brand/figure.toml`.
//! - `layout`: crate names against paths, dependency direction, licences, banned names, and
//!   every generated file fresh. Fails with every problem listed.
//! - `check`: the whole gate, in CI's order: fmt, clippy (native and wasm32), nextest, doc tests,
//!   `cargo deny`, the pen's tests on wasm32-wasip2, the browser pen's release build, then
//!   `layout`. Runs every step and fails with every failed step listed.

use std::error::Error as _;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

mod brand;
mod check;
mod diag;
mod error;
mod layout;
mod output;
mod process;
mod token;

use error::Error;
use output::{Line, Mode, Output};

/// What `cargo xtask` was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Task {
    Token,
    Diag,
    Brand,
    Layout,
    Check,
}

impl Task {
    /// Every task, in the order they are listed.
    pub(crate) const ALL: [Self; 5] = [Self::Token, Self::Diag, Self::Brand, Self::Layout, Self::Check];
    /// The tasks that write generated files.
    pub(crate) const GENERATORS: [Self; 3] = [Self::Token, Self::Diag, Self::Brand];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::Diag => "diag",
            Self::Brand => "brand",
            Self::Layout => "layout",
            Self::Check => "check",
        }
    }

    fn run(self, root: &Path) -> Result<Vec<Line>, Error> {
        let mut output = Output::new(root.to_path_buf(), Mode::Write);
        match self {
            Self::Token => token::write(&mut output)?,
            Self::Diag => diag::write(&mut output)?,
            Self::Brand => brand::write(&mut output)?,
            Self::Layout => return layout(root),
            Self::Check => return check::run(root),
        }
        Ok(output.report())
    }
}

/// The layout rules, then every generated file compared with what its generator writes now.
/// Every problem at once.
pub(crate) fn layout(root: &Path) -> Result<Vec<Line>, Error> {
    let mut output = Output::new(root.to_path_buf(), Mode::Check);
    let layout = layout::check(root);
    let token = token::write(&mut output);
    let diag = diag::write(&mut output);
    let brand = brand::write(&mut output);
    let failed: Vec<Error> = [layout, token, diag, brand, output.fresh()].into_iter().filter_map(Result::err).collect();
    if failed.is_empty() { Ok(output.report()) } else { Err(Error::Each(failed)) }
}

impl fmt::Display for Task {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for Task {
    type Err = Error;

    fn from_str(given: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|task| task.name() == given)
            .ok_or_else(|| Error::Task { given: Some(given.to_owned()) })
    }
}

/// An error and its causes, as `main` prints it: `error: ...`, then a `caused by: ...` line each.
struct Chain<'a>(&'a Error);

impl fmt::Display for Chain<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "error: {}", self.0)?;
        let mut cause = self.0.source();
        while let Some(error) = cause {
            write!(out, "\n  caused by: {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let task = std::env::args().nth(1).map_or(Err(Error::Task { given: None }), |arg| arg.parse::<Task>());
    let mut err = std::io::stderr();
    // A report that cannot be written fails the task: nothing it did goes unsaid.
    let written = match task.and_then(|task| task.run(&root)) {
        Ok(lines) => lines.iter().try_for_each(|line| writeln!(err, "{line}")).map(|()| ExitCode::SUCCESS),
        Err(error) => {
            error.into_each().iter().try_for_each(|error| writeln!(err, "{}", Chain(error))).map(|()| ExitCode::FAILURE)
        }
    };
    written.unwrap_or(ExitCode::FAILURE)
}
