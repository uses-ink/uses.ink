//! Everything a task can fail with. `Display` says what failed; the cause comes back from
//! `source()`, and `main` prints the chain.

use std::fmt;
use std::path::PathBuf;

use crate::Task;
use crate::layout::Problem;
use crate::process::Failure;

#[derive(Debug)]
pub(crate) enum Error {
    /// No task, or one that does not exist.
    Task {
        given: Option<String>,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
    /// A token file entry that is not a token.
    Token {
        name: String,
        flaw: crate::token::Flaw,
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
    Brand(crate::brand::Problem),
    /// Everything wrong with `diag/catalog.toml`.
    Catalog(Vec<crate::diag::Flaw>),
    Metadata(Failure),
    Layout(Vec<Problem>),
    /// Generated files that differ from what the generator writes now.
    Stale(Vec<PathBuf>),
    /// Several failures, each reported on its own.
    Each(Vec<Error>),
}

impl Error {
    /// Each failure on its own: the members of `Each`, or this one.
    pub(crate) fn into_each(self) -> Vec<Self> {
        match self {
            Self::Each(errors) => errors.into_iter().flat_map(Self::into_each).collect(),
            error => vec![error],
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Task { given } => {
                if let Some(given) = given {
                    write!(out, "no task `{given}`; ")?;
                }
                write!(out, "the tasks are {}", Listed(&Task::ALL))
            }
            Self::Read { path, .. } => write!(out, "cannot read {}", path.display()),
            Self::Write { path, .. } => write!(out, "cannot write {}", path.display()),
            Self::Toml { path, .. } => write!(out, "{} is not the TOML expected", path.display()),
            Self::Token { name, flaw } => write!(out, "token/token.toml: `{name}` {flaw}"),
            Self::Generated { path, .. } => write!(out, "{}: generated code does not parse", path.display()),
            Self::Rustfmt { path, .. } => write!(out, "rustfmt on {} failed", path.display()),
            Self::Brand(problem) => write!(out, "brand/figure.toml: {problem}"),
            Self::Catalog(flaws) => {
                write!(out, "diag/catalog.toml has {} flaw(s):", flaws.len())?;
                flaws.iter().try_for_each(|flaw| write!(out, "\n  {flaw}"))
            }
            Self::Metadata(_) => write!(out, "`cargo metadata` failed"),
            Self::Layout(problems) => {
                write!(out, "{} layout problem(s):", problems.len())?;
                problems.iter().try_for_each(|p| write!(out, "\n  {p}"))
            }
            Self::Stale(paths) => {
                let run: Vec<Run> = Task::GENERATORS.into_iter().map(Run).collect();
                write!(out, "stale generated files; run {}:", Listed(&run))?;
                paths.iter().try_for_each(|p| write!(out, "\n  {}", p.display()))
            }
            Self::Each(errors) => write!(out, "{} failures", errors.len()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Toml { source, .. } => Some(source),
            Self::Generated { source, .. } => Some(source),
            Self::Rustfmt { source, .. } | Self::Metadata(source) => Some(source),
            // Each of these is the whole failure: its data says what is wrong.
            Self::Task { .. }
            | Self::Token { .. }
            | Self::Brand(_)
            | Self::Catalog(_)
            | Self::Layout(_)
            | Self::Stale(_)
            | Self::Each(_) => None,
        }
    }
}

/// A task as the command that runs it: `cargo xtask token`.
struct Run(Task);

impl fmt::Display for Run {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "cargo xtask {}", self.0)
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
