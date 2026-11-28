//! `cargo xtask layout`: every [`Rule`] over the [`Workspace`], then every generated file
//! compared with what its generator writes now. The rules are the checkable parts of
//! docs/conventions.md: the path predicts the name, dependencies flow one way and from a layer only
//! to its own and earlier layers, licences match the crate's kind, no banned names, unsafe code only where a decision says, SQL text only through
//! the statement builders, function names of one or two words, no free functions, and release
//! files that name the binary that exists.

pub(crate) mod config;
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

pub(crate) use config::Config;
pub(crate) use model::Workspace;
pub(crate) use release::{Drift, Release};
pub(crate) use rules::{Alias, Banned, Fanout, Found, Free, Layer, Licence, Names, Order, Sql, Unsafe, Words};

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

/// A rule that fails only the crates `enforced` lists in `.config/layout.toml`; another crate's problems are counted, so
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
                Some(krate) if !workspace.config.enforces(krate) => {
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
type Every = (Names, Order, Layer, Licence, Alias, Banned, Unsafe, Sql, Gradual<Words>, Gradual<Free>, Release, Fanout);

impl Layout {
    /// Every rule, in the order their problems are listed.
    const RULES: Every =
        (Names, Order, Layer, Licence, Alias, Banned, Unsafe, Sql, Gradual(Words), Gradual(Free), Release, Fanout);
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
    /// Directories over the fan-out limit, with their entries, when that only reports.
    crowded: Vec<(PathBuf, usize)>,
    /// The fan-out limit they are over.
    limit: usize,
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
        if !self.crowded.is_empty() {
            let mut largest = self.crowded.clone();
            largest.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let count = largest.len();
            largest.truncate(5);
            report.line(Line::Crowded { count, limit: self.limit, largest });
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
    /// A dependency on a layer the crate's own layer may not take.
    Layer(Reach),
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
    /// A directory with more entries than the fan-out limit.
    Crowded {
        dir: PathBuf,
        entries: usize,
        limit: usize,
    },
}

/// A dependency that reaches into a layer its crate may not take.
#[derive(Debug)]
pub(crate) struct Reach {
    pub(crate) krate: String,
    pub(crate) on: String,
    pub(crate) from: String,
    pub(crate) to: String,
}

impl fmt::Display for Reach {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { krate, on, from, to } = self;
        write!(
            out,
            "`{krate}` in layer `{from}` may not depend on `{on}` in layer `{to}`: layers depend on earlier ones"
        )
    }
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
            | Self::Layer(Reach { krate, .. })
            | Self::Fixture { krate }
            | Self::Licence { krate, .. }
            | Self::Copyleft { krate, .. }
            | Self::Alias { krate } => Some(krate),
            Self::Banned { .. }
            | Self::Unsafe { .. }
            | Self::Sql { .. }
            | Self::Lexed { .. }
            | Self::Release(_)
            | Self::Crowded { .. } => None,
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
            Self::Layer(reach) => write!(out, "{reach}"),
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
                "`{}` holds {} code; only the files `unsafe.allowed` lists in {} may",
                path.display(),
                Unsafe::KEYWORD,
                Config::PATH
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
            Self::Crowded { dir, entries, limit } => {
                write!(out, "`{}` holds {entries} entries; the limit is {limit}", dir.display())
            }
        }
    }
}
