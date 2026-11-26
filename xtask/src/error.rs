//! Everything a task can fail with.

use std::fmt;
use std::path::PathBuf;

use crate::layout::Problem;

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
        why: &'static str,
    },
    /// Generated Rust that does not parse: a bug in the generator.
    Generated {
        path: PathBuf,
        source: syn::Error,
    },
    Rustfmt {
        path: PathBuf,
        why: String,
    },
    Brand(crate::brand::Problem),
    /// Everything wrong with `diag/catalog.toml`.
    Catalog(Vec<crate::diag::Flaw>),
    Metadata {
        why: String,
    },
    Layout(Vec<Problem>),
    /// Generated files that differ from what the generator writes now.
    Stale(Vec<PathBuf>),
    /// Several failures, each reported.
    Each(Vec<Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Task { given } => {
                if let Some(given) = given {
                    write!(out, "no task `{given}`; ")?;
                }
                write!(out, "the tasks are `token`, `diag`, `brand` and `check`")
            }
            Self::Read { path, source } => write!(out, "reading {}: {source}", path.display()),
            Self::Write { path, source } => write!(out, "writing {}: {source}", path.display()),
            Self::Toml { path, source } => write!(out, "{}: {source}", path.display()),
            Self::Token { name, why } => write!(out, "token/token.toml: `{name}` {why}"),
            Self::Generated { path, source } => {
                write!(out, "{}: generated code does not parse: {source}", path.display())
            }
            Self::Rustfmt { path, why } => write!(out, "rustfmt on {}: {why}", path.display()),
            Self::Brand(problem) => write!(out, "brand/figure.toml: {problem}"),
            Self::Catalog(flaws) => {
                write!(out, "diag/catalog.toml has {} flaw(s):", flaws.len())?;
                flaws.iter().try_for_each(|flaw| write!(out, "\n  {flaw}"))
            }
            Self::Metadata { why } => write!(out, "cargo metadata: {why}"),
            Self::Layout(problems) => {
                write!(out, "{} layout problem(s):", problems.len())?;
                problems.iter().try_for_each(|p| write!(out, "\n  {p}"))
            }
            Self::Stale(paths) => {
                write!(
                    out,
                    "stale generated files; run `cargo xtask token`, `cargo xtask diag` and `cargo xtask brand`:"
                )?;
                paths.iter().try_for_each(|p| write!(out, "\n  {}", p.display()))
            }
            Self::Each(errors) => errors
                .iter()
                .enumerate()
                .try_for_each(|(i, e)| write!(out, "{}{e}", if i == 0 { "" } else { "\nerror: " })),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Toml { source, .. } => Some(source),
            Self::Generated { source, .. } => Some(source),
            _ => None,
        }
    }
}
