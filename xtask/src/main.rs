//! Repo tasks, run as `cargo xtask <task>`:
//!
//! - `token`: write `token/src/generated.rs` from `token/token.toml`.
//! - `brand`: draw the logo, mark and favicon into `brand/` from `brand/figure.toml`.
//! - `check`: crate names against paths, dependency direction, licences, banned names, and
//!   every generated file fresh. Fails with every problem listed.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod brand;
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
    Brand,
    Check,
}

impl Task {
    fn parse(arg: Option<&str>) -> Result<Self, Error> {
        match arg {
            Some("token") => Ok(Self::Token),
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
            Self::Brand => brand::write(&mut output)?,
            Self::Check => {
                // Every problem at once: layout, then generated files.
                let layout = layout::check(root);
                token::write(&mut output)?;
                brand::write(&mut output)?;
                let failed: Vec<Error> = [layout, output.fresh()].into_iter().filter_map(Result::err).collect();
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
    match result {
        Ok(output) => {
            for line in output.report() {
                let _ignored = writeln!(err, "{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let _ignored = writeln!(err, "error: {error}");
            ExitCode::FAILURE
        }
    }
}
