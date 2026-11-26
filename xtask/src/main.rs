//! Repo tasks, run as `cargo xtask <task>`:
//!
//! - `token`: write `token/src/generated.rs` and `token/token.css` from `token/token.toml`.
//! - `diag`: write `model/src/generated.rs` and `diag/src/generated.rs` from `diag/catalog.toml`.
//! - `brand`: draw the logo, mark and favicon into `brand/` from `brand/figure.toml`.
//! - `check`: crate names against paths, dependency direction, licences, banned names, and
//!   every generated file fresh. Fails with every problem listed.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod brand;
mod diag;
mod error;
mod layout;
mod output;
mod token;

use error::Error;
use output::{Mode, Output};

/// What `cargo xtask` was asked to do.
#[derive(Debug, Clone, Copy)]
enum Task {
    Token,
    Diag,
    Brand,
    Check,
}

impl Task {
    fn parse(arg: Option<&str>) -> Result<Self, Error> {
        match arg {
            Some("token") => Ok(Self::Token),
            Some("diag") => Ok(Self::Diag),
            Some("brand") => Ok(Self::Brand),
            Some("check") => Ok(Self::Check),
            other => Err(Error::Task { given: other.map(str::to_owned) }),
        }
    }

    fn run(self, root: &Path) -> Result<Output, Error> {
        let mode = if matches!(self, Self::Check) { Mode::Check } else { Mode::Write };
        let mut output = Output::new(root.to_path_buf(), mode);
        match self {
            Self::Token => token::write(&mut output)?,
            Self::Diag => diag::write(&mut output)?,
            Self::Brand => brand::write(&mut output)?,
            Self::Check => {
                // Every problem at once: layout, then generated files.
                let layout = layout::check(root);
                let token = token::write(&mut output);
                let diag = diag::write(&mut output);
                let brand = brand::write(&mut output);
                let failed: Vec<Error> =
                    [layout, token, diag, brand, output.fresh()].into_iter().filter_map(Result::err).collect();
                if !failed.is_empty() {
                    return Err(Error::Each(failed));
                }
            }
        }
        Ok(output)
    }
}

fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let arg = std::env::args().nth(1);
    let result = Task::parse(arg.as_deref()).and_then(|task| task.run(&root));
    let mut err = std::io::stderr();
    // A report that cannot be written fails the task: nothing it did goes unsaid.
    let (lines, code) = match result {
        Ok(output) => (output.report(), ExitCode::SUCCESS),
        Err(error) => (vec![format!("error: {error}")], ExitCode::FAILURE),
    };
    match lines.iter().try_for_each(|line| writeln!(err, "{line}")) {
        Ok(()) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
