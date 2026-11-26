//! `cargo xtask layout`: every [`Rule`] over the [`Workspace`], then every generated file
//! compared with what its generator writes now. The rules are the checkable parts of
//! docs/conventions.md: the path predicts the name, dependencies flow one way, licences match the
//! crate's kind, no banned names, unsafe code only where a decision says, function names of one
//! or two words, and no free functions.

mod file;
mod manifest;
mod syntax;
mod workspace;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::brand::Brand;
use crate::check::Step;
use crate::diag::Diag;
use crate::error::Error;
use crate::output::{Generators, Line, Mode, Output, Report};
use crate::token::Token;

pub(crate) use file::{Banned, Unsafe};
pub(crate) use manifest::{Alias, Licence, Names, Order};
pub(crate) use syntax::{Free, Words};
pub(crate) use workspace::Workspace;

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
    "uses-ink-fixture",
    "uses-ink-format-markdown",
    "uses-ink-host-native",
    "uses-ink-theme-quiet",
    "uses-ink-token",
    "xtask",
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

impl Layout {
    /// Every rule, in the order their problems are listed.
    const RULES: (Names, Order, Licence, Alias, Banned, Unsafe, Gradual<Words>, Gradual<Free>) =
        (Names, Order, Licence, Alias, Banned, Unsafe, Gradual(Words), Gradual(Free));
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
    /// A Rust file that is not Rust tokens, so no rule can read it.
    Lexed {
        path: PathBuf,
        source: proc_macro2::LexError,
    },
    /// A function or method name of three words or more.
    Words(Function),
    /// A free function outside a crate's entry points.
    Free(Function),
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
            | Self::Alias { krate } => Some(krate),
            Self::Banned { .. } | Self::Unsafe { .. } | Self::Lexed { .. } => None,
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
            Self::Alias { krate } => write!(out, "`{krate}` has no alias in [workspace.dependencies]"),
            Self::Banned { path } => write!(out, "`{}` uses a banned name", path.display()),
            Self::Unsafe { path } => write!(
                out,
                "`{}` holds {} code; only {} may (xtask's Unsafe::ALLOWED)",
                path.display(),
                Unsafe::KEYWORD,
                Unsafe::ALLOWED.join(", ")
            ),
            Self::Lexed { path, source } => write!(out, "`{}` is not Rust tokens: {source}", path.display()),
            Self::Words(Function { path, name, .. }) => {
                write!(out, "`{}`: `{name}` is three words or more; it is a missing type or module", path.display())
            }
            Self::Free(Function { path, name, .. }) => {
                write!(out, "`{}`: `{name}` is a free function; put it on the type it acts on", path.display())
            }
        }
    }
}
