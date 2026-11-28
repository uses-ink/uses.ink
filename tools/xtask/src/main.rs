//! Repo tasks, run as `cargo xtask <task>`:
//!
//! - `token`: write `crates/core/token/src/generated.rs` and `crates/core/token/token.css` from
//!   `crates/core/token/token.toml`.
//! - `diag`: write `crates/core/model/src/generated.rs` and `crates/core/diag/src/generated.rs` from
//!   `crates/core/diag/catalog.toml`.
//! - `brand`: draw the logo, mark and favicon into `brand/` from `brand/figure.toml`.
//! - `layout`: every layout rule over the workspace, and every generated file fresh. Fails with
//!   every problem listed.
//! - `docs`: write the decisions index, `docs/decisions/README.md`, from the front matter of the
//!   decision files, and the crate index, `docs/crates.md`, from the manifests.
//! - `component`: build the plugins' guest crates for `wasm32-unknown-unknown` and wrap each
//!   into a component under `target/plugin/`.
//! - `worker <case>`: a Worker run in `workerd` through `wrangler dev --local`, with no Cloudflare
//!   account: the hosted spikes and the conformance suites (`probe`, `lock`, `api`, `store`, `db`, `blob`, `rig`).
//! - `check [stage]`: the whole gate, or one of its stages (`lint`, `test`, `docs`, `wasm`,
//!   `sites`, `worker`), which CI runs as parallel jobs: fmt, clippy (native and wasm32), `cargo deny` and
//!   `layout`; the components and nextest; the docs checks (links, citations, crate paths and decisions)
//!   and doc tests; the pen on wasm32-wasip2 and in the browser;
//!   `ink build --check` on every CLI site fixture. Runs every step and fails with every failed
//!   step listed.
//!
//! Each task is a type: the generators implement [`generate::output::Generator`], the gate's steps
//! [`check::Step`], the layout rules [`layout::Rule`].

use std::error::Error as _;
use std::fmt;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

/// Implements a list trait for tuples of its members, so a list of different types runs in
/// order with static dispatch. The body says what is done with each member:
///
/// ```ignore
/// tuples! { Rules: Rule, fn check(&self, workspace: &Workspace, problems: &mut Problems) { |rule| rule.check(workspace, problems) } }
/// ```
///
/// implements `Rules` for `(A,)`, `(A, B)` and so on up to twelve members, each a `Rule`.
macro_rules! tuples {
    ($list:ident: $member:ident, fn $method:ident(&self $(, $arg:ident: $ty:ty)*) { |$item:ident| $body:expr }) => {
        tuples!(@each $list $member $method ($($arg: $ty),*) ($item $body) [] A B C D E F G H I J K L);
    };
    (@each $list:ident $member:ident $method:ident $args:tt $call:tt [$($done:ident)*] $next:ident $($rest:ident)*) => {
        tuples!(@impl $list $member $method $args $call $($done)* $next);
        tuples!(@each $list $member $method $args $call [$($done)* $next] $($rest)*);
    };
    (@each $list:ident $member:ident $method:ident $args:tt $call:tt [$($done:ident)*]) => {};
    (@impl $list:ident $member:ident $method:ident ($($arg:ident: $ty:ty),*) ($item:ident $body:expr) $($t:ident)+) => {
        #[expect(non_snake_case, reason = "each binding is named for its type parameter")]
        impl<$($t: $member),+> $list for ($($t,)+) {
            fn $method(&self, $($arg: $ty),*) {
                let ($($t,)+) = self;
                $( { let $item = $t; $body; } )+
            }
        }
    };
}

mod build;
mod check;
mod docs;
mod error;
mod generate;
mod layout;
mod process;
mod report;
mod root;
mod worker;

use error::Error;
use generate::output::{Mode, Output};
use report::Report;
use root::Root;

/// What `cargo xtask` was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Task {
    Token,
    Diag,
    Brand,
    Layout,
    Docs,
    Component,
    Worker,
    Check,
}

impl Task {
    /// Every task, in the order they are listed.
    pub(crate) const ALL: [Self; 8] =
        [Self::Token, Self::Diag, Self::Brand, Self::Layout, Self::Docs, Self::Component, Self::Worker, Self::Check];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::Diag => "diag",
            Self::Brand => "brand",
            Self::Layout => "layout",
            Self::Docs => "docs",
            Self::Component => "component",
            Self::Worker => "worker",
            Self::Check => "check",
        }
    }

    /// What the tasks that take an argument, or that refuse one, report; none for the rest.
    fn argued(self, root: &Path, argument: Option<&str>) -> Option<Report> {
        Some(match (self, argument) {
            (Self::Check, given) => match given.map(str::parse::<check::Stage>).transpose() {
                Ok(stage) => check::Gate::new(root).run(stage),
                Err(error) => Report::from(error),
            },
            (Self::Worker, Some(given)) => match given.parse::<worker::Case>() {
                Ok(case) => {
                    let mut report = Report::default();
                    check::Step::run(&worker::Worker(case), root, &mut report);
                    report
                }
                Err(error) => Report::from(error),
            },
            (Self::Worker, None) => Report::from(Error::Case { given: String::new() }),
            (_, Some(_)) => Report::from(Error::Stageless { task: self }),
            (_, None) => return None,
        })
    }

    /// Runs it from the workspace at `root`, with the argument after its name: a stage of `check`,
    /// a case of `worker`; the report holds every line and every failure.
    fn run(self, root: &Path, argument: Option<&str>) -> Report {
        if let Some(done) = self.argued(root, argument) {
            return done;
        }
        let mut report = Report::default();
        let mut output = Output::new(root, Mode::Write);
        let generated = match self {
            Self::Token => output.generate(&generate::token::Token),
            Self::Diag => output.generate(&generate::diag::Diag),
            Self::Brand => output.generate(&generate::brand::Brand),
            Self::Layout => {
                check::Step::run(&layout::Layout, root, &mut report);
                return report;
            }
            Self::Docs => {
                docs::Generated::write(root, &mut report);
                return report;
            }
            Self::Component => {
                check::Step::run(&build::component::Components, root, &mut report);
                return report;
            }
            Self::Worker | Self::Check => return report,
        };
        report.take(generated);
        output.settle(&mut report);
        report
    }
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
            .ok_or_else(|| Error::Unknown { given: given.to_owned() })
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
    let mut args = std::env::args().skip(1);
    let report = match (Root::find(), args.next().map(|arg| arg.parse::<Task>()), args.next()) {
        (Ok(root), Some(Ok(task)), argument) => task.run(&root, argument.as_deref()),
        (Err(error), ..) | (Ok(_), Some(Err(error)), _) => Report::from(error),
        (Ok(_), None, _) => Report::from(Error::Missing),
    };
    let mut err = std::io::stderr();
    // A report that cannot be written fails the task: nothing it did goes unsaid.
    let written = report
        .lines()
        .iter()
        .try_for_each(|line| writeln!(err, "{line}"))
        .and_then(|()| report.failures().iter().try_for_each(|error| writeln!(err, "{}", Chain(error))));
    match written {
        Ok(()) if report.failures().is_empty() => ExitCode::SUCCESS,
        Ok(()) | Err(_) => ExitCode::FAILURE,
    }
}
