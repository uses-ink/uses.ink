//! What a task did: its [`Report`] of [`Line`]s and failures.

use std::fmt;
use std::path::PathBuf;

use crate::error::Error;

/// What a task did: a line each for what went well, and every failure.
#[derive(Debug, Default)]
pub(crate) struct Report {
    lines: Vec<Line>,
    failures: Vec<Error>,
}

impl Report {
    /// Adds a line.
    pub(crate) fn line(&mut self, line: Line) {
        self.lines.push(line);
    }

    /// Adds a failure.
    pub(crate) fn fail(&mut self, error: Error) {
        self.failures.push(error);
    }

    /// Adds the failure of `result`, if it failed.
    pub(crate) fn take(&mut self, result: Result<(), Error>) {
        if let Err(error) = result {
            self.fail(error);
        }
    }

    pub(crate) fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub(crate) fn failures(&self) -> &[Error] {
        &self.failures
    }
}

impl From<Error> for Report {
    fn from(error: Error) -> Self {
        Self { lines: Vec::new(), failures: vec![error] }
    }
}

/// One line of a task's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    /// A generated file written, by path relative to the root.
    Wrote(PathBuf),
    /// A file in a claimed directory removed because nothing generates it any more.
    Removed(PathBuf),
    /// Every generated file already held what the generator writes.
    Unchanged,
    /// A crate breaking a rule that is not enforced on it yet, and how often.
    Waived { krate: String, rule: &'static str, count: usize },
    /// The layout and every generated file are as they should be.
    Checked,
    /// A site fixture built the same deploy natively and through the components.
    Matched(PathBuf),
    /// Every step of the gate passed.
    Passed,
}

impl fmt::Display for Line {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wrote(path) => write!(out, "wrote {}", path.display()),
            Self::Removed(path) => write!(out, "removed {} (no longer generated)", path.display()),
            Self::Unchanged => write!(out, "nothing changed"),
            Self::Waived { krate, rule, count } => {
                write!(out, "note: `{rule}` finds {count} in `{krate}`, not enforced there yet (xtask's ENFORCED)")
            }
            Self::Checked => write!(out, "layout fine, generated files fresh"),
            Self::Matched(path) => write!(out, "{}: same deploy natively and as components", path.display()),
            Self::Passed => write!(out, "check: every step passed"),
        }
    }
}
