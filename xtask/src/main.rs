//! Repo tasks, run as `cargo xtask <task>`:
//!
//! - `token`: write `token/src/generated.rs` and `token/token.css` from `token/token.toml`.
//! - `diag`: write `model/src/generated.rs` and `diag/src/generated.rs` from `diag/catalog.toml`.
//! - `brand`: draw the logo, mark and favicon into `brand/` from `brand/figure.toml`.
//! - `layout`: every layout rule over the workspace, and every generated file fresh. Fails with
//!   every problem listed.
//! - `component`: build the plugins' guest crates for `wasm32-unknown-unknown` and wrap each
//!   into a component under `target/plugin/`.
//! - `check [stage]`: the whole gate, or one of its stages (`lint`, `test`, `docs`, `wasm`,
//!   `sites`), which CI runs as parallel jobs: fmt, clippy (native and wasm32), `cargo deny` and
//!   `layout`; the components and nextest; doc tests; the pen on wasm32-wasip2 and in the browser;
//!   `ink build --check` on every CLI site fixture. Runs every step and fails with every failed
//!   step listed.
//! - `farm <verb>`: the build farm client: the gate and the inner loop on the hosts of the
//!   user's config (`~/.config/uses-ink/farm.toml`), each job in a slot of its own. The verbs are
//!   `check`, `cargo`, `bless`, `status`, `workspace`, `wait` and `log`; docs/workflow.md has
//!   them, the config and the exit codes. `cargo xtask check` stays as it is, for CI and humans.
//!
//! Each task is a type: the generators implement [`output::Generator`], the gate's steps
//! [`check::Step`], the layout rules [`layout::Rule`].

use std::error::Error as _;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
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

mod brand;
mod check;
mod component;
mod diag;
mod error;
mod farm;
mod layout;
mod output;
mod process;
mod token;

use error::Error;
use output::{Mode, Output, Report};

/// What `cargo xtask` was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Task {
    Token,
    Diag,
    Brand,
    Layout,
    Component,
    Check,
    Farm,
}

impl Task {
    /// Every task, in the order they are listed.
    pub(crate) const ALL: [Self; 7] =
        [Self::Token, Self::Diag, Self::Brand, Self::Layout, Self::Component, Self::Check, Self::Farm];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::Diag => "diag",
            Self::Brand => "brand",
            Self::Layout => "layout",
            Self::Component => "component",
            Self::Check => "check",
            Self::Farm => "farm",
        }
    }

    /// Runs it from the workspace at `root`; the report holds every line and every failure.
    fn run(self, root: &Path, stage: Option<check::Stage>) -> Report {
        if let (Some(_), false) = (stage, self == Self::Check) {
            return Report::from(Error::Stageless { task: self });
        }
        let mut report = Report::default();
        let mut output = Output::new(root, Mode::Write);
        let generated = match self {
            Self::Token => output.generate(&token::Token),
            Self::Diag => output.generate(&diag::Diag),
            Self::Brand => output.generate(&brand::Brand),
            Self::Layout => {
                check::Step::run(&layout::Layout, root, &mut report);
                return report;
            }
            Self::Component => {
                check::Step::run(&component::Components, root, &mut report);
                return report;
            }
            Self::Check => return check::Gate::new(root).run(stage),
            // `main` hands `farm` its whole command line before a task is parsed
            Self::Farm => return Report::from(Error::Missing),
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some(Task::Farm.name()) {
        return farm::Entry::main(&root, args.collect());
    }
    let mut args = std::env::args().skip(1);
    let report = match (args.next().map(|arg| arg.parse::<Task>()), args.next().map(|arg| arg.parse::<check::Stage>()))
    {
        (Some(Ok(task)), None) => task.run(&root, None),
        (Some(Ok(task)), Some(Ok(stage))) => task.run(&root, Some(stage)),
        (Some(Err(error)), _) | (Some(Ok(_)), Some(Err(error))) => Report::from(error),
        (None, _) => Report::from(Error::Missing),
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
