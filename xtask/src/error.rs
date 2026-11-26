//! Everything a task can fail with. `Display` says what failed; the cause comes back from
//! `source()`, and `main` prints the chain.

use std::fmt;
use std::path::PathBuf;

use crate::Task;
use crate::check::{Invocation, Tool};
use crate::layout::Problem;
use crate::output::Stale;
use crate::process::Failure;

/// One failure of a task.
#[derive(Debug)]
pub(crate) enum Error {
    /// No task given.
    Missing,
    /// A task that does not exist.
    Unknown {
        given: String,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A generator's source file that is not what it should be.
    Source {
        path: &'static str,
        flaw: Flaw,
    },
    /// Generated Rust that does not parse: a bug in the generator.
    Generated {
        path: PathBuf,
        source: syn::Error,
    },
    Rustfmt {
        path: PathBuf,
        source: Failure,
    },
    /// `cargo metadata` failed.
    Metadata(Failure),
    /// The workspace manifest is not the TOML expected.
    Manifest {
        path: PathBuf,
        source: toml::de::Error,
    },
    /// A module of the workspace that does not parse.
    Parse {
        path: PathBuf,
        source: syn::Error,
    },
    /// A `mod name;` whose file is in none of the places Rust looks.
    Module {
        path: PathBuf,
        name: String,
    },
    /// Everything the layout rules found wrong.
    Layout(Vec<Problem>),
    /// Generated files that differ from what their generator writes now.
    Stale(Vec<Stale>),
    /// A step of `cargo xtask check` that failed.
    Step {
        command: Invocation,
        source: Failure,
    },
    /// A step that needs a tool that is not installed.
    Tool {
        command: Invocation,
        tool: Tool,
    },
    /// Progress could not be written to stderr.
    Progress(std::io::Error),
}

/// What is wrong with a generator's source file.
#[derive(Debug)]
pub(crate) enum Flaw {
    /// Not TOML, or not the shape expected.
    Toml(toml::de::Error),
    /// An entry of `token/token.toml` that is not a token.
    Token(crate::token::Refusal),
    /// Everything wrong with `diag/catalog.toml`.
    Catalog(Vec<crate::diag::Flaw>),
    /// A drawing of `brand/figure.toml` that cannot be drawn.
    Brand(crate::brand::Problem),
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(out, "no task given; the tasks are {}", Listed(&Task::ALL)),
            Self::Unknown { given } => write!(out, "no task `{given}`; the tasks are {}", Listed(&Task::ALL)),
            Self::Read { path, .. } => write!(out, "cannot read {}", path.display()),
            Self::Write { path, .. } => write!(out, "cannot write {}", path.display()),
            Self::Source { path, flaw } => write!(out, "{path}: {flaw}"),
            Self::Generated { path, .. } => write!(out, "{}: generated code does not parse", path.display()),
            Self::Rustfmt { path, .. } => write!(out, "rustfmt on {} failed", path.display()),
            Self::Metadata(_) => write!(out, "`cargo metadata` failed"),
            Self::Manifest { path, .. } => write!(out, "{} is not the TOML expected", path.display()),
            Self::Parse { path, .. } => write!(out, "{} does not parse as a Rust file", path.display()),
            Self::Module { path, name } => write!(out, "{}: no file for `mod {name};`", path.display()),
            Self::Layout(problems) => write!(out, "layout problems:{}", Lines(problems)),
            Self::Stale(stale) => write!(out, "stale generated files:{}", Lines(stale)),
            Self::Step { command, .. } => write!(out, "check: `{command}` failed"),
            Self::Tool { command, tool } => write!(
                out,
                "check: `{command}` needs `{}`, which is not on PATH; install it with `{}`",
                tool.binary(),
                tool.install()
            ),
            Self::Progress(_) => write!(out, "cannot write progress to stderr"),
        }
    }
}

impl fmt::Display for Flaw {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(_) => write!(out, "not the TOML expected"),
            Self::Token(refusal) => write!(out, "{refusal}"),
            Self::Catalog(flaws) => write!(out, "flaws:{}", Lines(flaws)),
            Self::Brand(problem) => write!(out, "{problem}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } | Self::Progress(source) => Some(source),
            Self::Source { flaw: Flaw::Toml(source), .. } | Self::Manifest { source, .. } => Some(source),
            Self::Generated { source, .. } | Self::Parse { source, .. } => Some(source),
            Self::Rustfmt { source, .. } | Self::Metadata(source) | Self::Step { source, .. } => Some(source),
            // Each of these is the whole failure: its data says what is wrong.
            Self::Missing
            | Self::Unknown { .. }
            | Self::Source { .. }
            | Self::Module { .. }
            | Self::Layout(_)
            | Self::Stale(_)
            | Self::Tool { .. } => None,
        }
    }
}

impl From<toml::de::Error> for Flaw {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error)
    }
}

/// Items in backticks, as a sentence lists them: `a`, `b` and `c`.
pub(crate) struct Listed<'a, T>(pub(crate) &'a [T]);

impl<T: fmt::Display> fmt::Display for Listed<'_, T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last = self.0.len().saturating_sub(1);
        self.0.iter().enumerate().try_for_each(|(i, item)| {
            let sep = match i {
                0 => "",
                i if i == last => " and ",
                _ => ", ",
            };
            write!(out, "{sep}`{item}`")
        })
    }
}

/// Items below a heading, each on its own indented line.
pub(crate) struct Lines<'a, T>(pub(crate) &'a [T]);

impl<T: fmt::Display> fmt::Display for Lines<'_, T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|item| write!(out, "\n  {item}"))
    }
}
