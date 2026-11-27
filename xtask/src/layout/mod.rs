//! `cargo xtask layout`: every [`Rule`] over the [`Workspace`], then every generated file
//! compared with what its generator writes now. The rules are the checkable parts of
//! docs/conventions.md: the path predicts the name, dependencies flow one way, licences match the
//! crate's kind, no banned names, unsafe code only where a decision says, SQL text only through
//! the statement builders, function names of one or two words, no free functions, and release
//! files that name the binary that exists.

mod model;
mod read;
mod release;
mod rules;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::check::Step;
use crate::error::Error;
use crate::generate::brand::Brand;
use crate::generate::diag::Diag;
use crate::generate::output::{Generators, Mode, Output};
use crate::generate::token::Token;
use crate::report::{Line, Report};

pub(crate) use model::Workspace;
pub(crate) use release::{Drift, Release};
pub(crate) use rules::{Alias, Banned, Found, Free, Licence, Names, Order, Sql, Unsafe, Words};

/// One check over the whole workspace. A new check is a new rule.
pub(crate) trait Rule {
    /// Its name, as a note about it says.
    const NAME: &'static str;

    /// Adds every problem it finds to `problems`.
    fn check(&self, workspace: &Workspace, problems: &mut Problems);
}

/// A list of rules, each checked in order. Implemented for tuples of [`Rule`]s.
pub(crate) trait Rules {
    fn check(&self, workspace: &Workspace, problems: &mut Problems);
}

tuples! { Rules: Rule, fn check(&self, workspace: &Workspace, problems: &mut Problems) { |rule| rule.check(workspace, problems) } }

/// The crates a [`Gradual`] rule fails. The rest are counted in a note until their port lands
/// and they join this list.
pub(crate) const ENFORCED: &[&str] = &[
    "freehand",
    "freehand-web",
    "uses-ink-cli",
    "uses-ink-config",
    "uses-ink-diag",
    "uses-ink-clock",
    "uses-ink-edge",
    "uses-ink-edge-api",
    "uses-ink-edge-control",
    "uses-ink-watch",
    "uses-ink-watch-git",
    "uses-ink-edge-axum",
    "uses-ink-edge-folder",
    "uses-ink-engine",
    "uses-ink-fixture",
    "uses-ink-format-markdown",
    "uses-ink-format-obsidian",
    "uses-ink-host-image",
    "uses-ink-host-native",
    "uses-ink-host-wasmtime",
    "uses-ink-host-wasmtime-hostile",
    "uses-ink-model",
    "uses-ink-sdk",
    "uses-ink-sig",
    "uses-ink-source-git",
    "uses-ink-source-git-guest",
    "uses-ink-format-markdown-guest",
    "uses-ink-format-obsidian-guest",
    "uses-ink-theme-quiet-guest",
    "uses-ink-theme-quiet",
    "uses-ink-token",
    "uses-ink-trace",
    "uses-ink-blob",
    "uses-ink-blob-conformance",
    "uses-ink-blob-dir",
    "uses-ink-build",
    "uses-ink-build-conformance",
    "uses-ink-build-local",
    "uses-ink-control",
    "uses-ink-db",
    "uses-ink-db-conformance",
    "uses-ink-db-sqlite",
    "uses-ink-queue",
    "uses-ink-queue-conformance",
    "uses-ink-queue-local",
    "uses-ink-secret",
    "uses-ink-secret-conformance",
    "uses-ink-secret-local",
    "uses-ink-web",
    "uses-ink-mail",
    "uses-ink-mail-smtp",
    "uses-ink-https",
    "xtask",
    "farm",
];

/// A rule that fails only the crates in [`ENFORCED`]; another crate's problems are counted, so
/// the rule reports on every crate while the code moves onto it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gradual<R>(pub(crate) R);

impl<R: Rule> Rule for Gradual<R> {
    const NAME: &'static str = R::NAME;

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let mut own = Problems::default();
        self.0.check(workspace, &mut own);
        for problem in own.found {
            match problem.krate() {
                Some(krate) if !ENFORCED.contains(&krate) => {
                    *problems.waived.entry((krate.to_owned(), R::NAME)).or_default() += 1;
                }
                _ => problems.found.push(problem),
            }
        }
        problems.waived.extend(own.waived);
    }
}

/// The layout task, and the last step of the gate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout;

/// Every rule of the layout task, as a tuple.
type Every = (Names, Order, Licence, Alias, Banned, Unsafe, Sql, Gradual<Words>, Gradual<Free>, Release);

impl Layout {
    /// Every rule, in the order their problems are listed.
    const RULES: Every = (Names, Order, Licence, Alias, Banned, Unsafe, Sql, Gradual(Words), Gradual(Free), Release);
}

impl Step for Layout {
    fn run(&self, root: &Path, report: &mut Report) {
        let before = report.failures().len();
        let mut problems = Problems::default();
        match Workspace::read(root) {
            Ok(workspace) => Self::RULES.check(&workspace, &mut problems),
            Err(error) => report.fail(error),
        }
        problems.settle(report);
        let mut output = Output::new(root, Mode::Check);
        (Token, Diag, Brand).generate(&mut output, report);
        output.settle(report);
        if report.failures().len() == before {
            report.line(Line::Checked);
        }
    }
}

impl fmt::Display for Layout {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("cargo xtask layout")
    }
}

/// What the rules found: problems that fail, and per crate and rule the count of those waived.
#[derive(Debug, Default)]
pub(crate) struct Problems {
    found: Vec<Problem>,
    waived: BTreeMap<(String, &'static str), usize>,
}

impl Problems {
    /// What `rule` finds in `workspace`, for the tests.
    #[cfg(test)]
    pub(crate) fn of(rule: &impl Rule, workspace: &Workspace) -> Vec<Problem> {
        let mut problems = Self::default();
        rule.check(workspace, &mut problems);
        problems.found
    }

    pub(crate) fn push(&mut self, problem: Problem) {
        self.found.push(problem);
    }

    /// Adds the found problems to `report` as one failure, the waived as a note each.
    fn settle(self, report: &mut Report) {
        for ((krate, rule), count) in self.waived {
            report.line(Line::Waived { krate, rule, count });
        }
        if !self.found.is_empty() {
            report.fail(Error::Layout(self.found));
        }
    }
}

/// One thing a rule found wrong.
#[derive(Debug)]
pub(crate) enum Problem {
    /// A crate whose name its path does not predict.
    Name {
        krate: String,
        path: String,
        expected: String,
    },
    /// A crate with no place in the dependency order.
    Unordered {
        krate: String,
    },
    /// A dependency the order does not allow.
    Dependency {
        krate: String,
        on: String,
    },
    /// The fixture harness taken as a normal or build dependency.
    Fixture {
        krate: String,
    },
    Licence {
        krate: String,
        found: Option<String>,
        expected: &'static str,
    },
    /// A permissive crate depending on a crate of ours under the service's licence.
    Copyleft {
        krate: String,
        on: String,
    },
    /// A crate another uses that has no alias in `[workspace.dependencies]`.
    Alias {
        krate: String,
    },
    Banned {
        path: PathBuf,
    },
    Unsafe {
        path: PathBuf,
    },
    /// SQL text in code, a raw escape of the statement builders, or a `.sql` file.
    Sql {
        path: PathBuf,
        found: Found,
    },
    /// A Rust file that is not Rust tokens, so no rule can read it.
    Lexed {
        path: PathBuf,
        source: proc_macro2::LexError,
    },
    /// A function or method name of three words or more.
    Words(Function),
    /// A free function outside a crate's entry points.
    Free(Function),
    /// Release files that disagree with the workspace or with each other.
    Release(Drift),
}

/// A function a syntax rule names: where it is, and what it is called.
#[derive(Debug)]
pub(crate) struct Function {
    pub(crate) krate: String,
    pub(crate) path: PathBuf,
    pub(crate) name: String,
}

impl Problem {
    /// The crate it is in, for a problem that belongs to one crate's code.
    fn krate(&self) -> Option<&str> {
        match self {
            Self::Words(function) | Self::Free(function) => Some(&function.krate),
            Self::Name { krate, .. }
            | Self::Unordered { krate }
            | Self::Dependency { krate, .. }
            | Self::Fixture { krate }
            | Self::Licence { krate, .. }
            | Self::Copyleft { krate, .. }
            | Self::Alias { krate } => Some(krate),
            Self::Banned { .. } | Self::Unsafe { .. } | Self::Sql { .. } | Self::Lexed { .. } | Self::Release(_) => {
                None
            }
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { krate, path, expected } => write!(out, "`{krate}` at `{path}/` should be named `{expected}`"),
            Self::Unordered { krate } => {
                write!(out, "`{krate}` has no row in xtask's MAY_USE: decide what it may depend on")
            }
            Self::Dependency { krate, on } => write!(out, "`{krate}` may not depend on `{on}`"),
            Self::Fixture { krate } => {
                write!(out, "`{krate}` takes `{}` outside [dev-dependencies]; it is for tests only", Order::FIXTURE)
            }
            Self::Licence { krate, found, expected } => {
                write!(out, "`{krate}` is licensed `{}`, expected `{expected}`", found.as_deref().unwrap_or("nothing"))
            }
            Self::Copyleft { krate, on } => write!(
                out,
                "`{krate}` is {} but depends on `{on}`, which is not: whoever links it would take the AGPL",
                Licence::PERMISSIVE
            ),
            Self::Alias { krate } => write!(out, "`{krate}` has no alias in [workspace.dependencies]"),
            Self::Banned { path } => write!(out, "`{}` uses a banned name", path.display()),
            Self::Unsafe { path } => write!(
                out,
                "`{}` holds {} code; only {} may (xtask's Unsafe::ALLOWED)",
                path.display(),
                Unsafe::KEYWORD,
                Unsafe::ALLOWED.join(", ")
            ),
            Self::Sql { path, found } => write!(out, "`{}` {found}", path.display()),
            Self::Lexed { path, source } => write!(out, "`{}` is not Rust tokens: {source}", path.display()),
            Self::Words(Function { path, name, .. }) => {
                write!(out, "`{}`: `{name}` is three words or more; it is a missing type or module", path.display())
            }
            Self::Free(Function { path, name, .. }) => {
                write!(out, "`{}`: `{name}` is a free function; put it on the type it acts on", path.display())
            }
            Self::Release(drift) => write!(out, "release: {drift}"),
        }
    }
}
