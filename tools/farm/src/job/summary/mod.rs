//! A job's log, summarised: the test counts and every failing test with its output, the compiler
//! diagnostics, the gate's `check:` lines. The summary says what went wrong in a screen; the full
//! log stays on disk. It reads what nextest, cargo and `cargo xtask check` print with colour off
//! (`CARGO_TERM_COLOR=never`).

mod diagnostic;
mod suite;
#[cfg(test)]
mod tests;

use std::fmt;

use serde::Serialize;

pub(crate) use diagnostic::Diagnostics;
pub(crate) use suite::Tests;
/// The most lines of one failing test's output that are shown.
pub(crate) const OUTPUT_LINES: usize = 60;
/// The most diagnostics that are shown when nothing failed to compile.
pub(crate) const DIAGNOSTICS: usize = 10;
/// The most lines of the first error that are shown: its head, then its code (the `-->` line
/// is dropped: the head has the place).
pub(crate) const LEAD: usize = 11;
/// The other diagnostics shown after the first error; the rest is a count.
pub(crate) const ALSO: usize = 2;

/// Everything a log says about how it went.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Summary {
    pub(crate) tests: Option<Tests>,
    pub(crate) diagnostics: Diagnostics,
    /// The gate's `check:` lines, and the error chains it printed.
    pub(crate) checks: Vec<String>,
}

impl Summary {
    /// Reads a whole log.
    pub(crate) fn of(log: &str) -> Self {
        Self { tests: Tests::of(log), diagnostics: Diagnostics::of(log), checks: Self::checks(log) }
    }

    fn checks(log: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut inside = false;
        for line in log.lines() {
            let chain = ["error: check:", "error: layout", "error: stale", "error: no stage", "error: no task"]
                .iter()
                .any(|start| line.starts_with(start));
            inside = chain || (inside && line.starts_with("  "));
            if line.starts_with("check: ") || inside {
                found.push(line.to_owned());
            }
        }
        found
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.checks {
            writeln!(out, "{line}")?;
        }
        if let Some(tests) = &self.tests {
            write!(out, "{tests}")?;
        }
        let Diagnostics { errors, warnings, first, lead } = &self.diagnostics;
        if *errors + *warnings > 0 {
            writeln!(out, "compiler: {errors} errors, {warnings} warnings")?;
            // a failure shows its first error whole and two more in short; no more lines than that
            let head = usize::from(!lead.is_empty());
            let room = if head == 1 { ALSO } else { DIAGNOSTICS };
            let rest: Vec<&String> = first.iter().skip(head).take(room).collect();
            lead.iter().chain(rest.iter().copied()).try_for_each(|line| writeln!(out, "  {line}"))?;
            let shown = u32::try_from(head + rest.len()).unwrap_or(u32::MAX);
            if errors + warnings > shown {
                writeln!(out, "  ... {} more in the log", errors + warnings - shown)?;
            }
        }
        Ok(())
    }
}
