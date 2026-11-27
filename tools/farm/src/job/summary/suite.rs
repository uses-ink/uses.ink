//! Nextest and test-harness output read into counts and failures.

use std::fmt;

use serde::Serialize;

use super::OUTPUT_LINES;

/// A nextest run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Tests {
    pub(crate) run: u32,
    pub(crate) passed: u32,
    pub(crate) failed: u32,
    pub(crate) timed_out: u32,
    pub(crate) skipped: u32,
    pub(crate) failures: Vec<Failure>,
}

/// A test that did not pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Failure {
    pub(crate) status: String,
    pub(crate) name: String,
    /// Its output, at most [`OUTPUT_LINES`] lines.
    pub(crate) output: Vec<String>,
    /// How many lines of it were cut.
    pub(crate) cut: usize,
}

/// The words nextest starts a status line with.
const STATUSES: [&str; 22] = [
    "PASS",
    "FAIL",
    "TIMEOUT",
    "SLOW",
    "LEAK",
    "LEAK-FAIL",
    "FLAKY",
    "START",
    "SKIP",
    "RETRY",
    "TERMINATING",
    "SIGABRT",
    "SIGSEGV",
    "SIGKILL",
    "SIGTERM",
    "SIGINT",
    "SIGBUS",
    "SIGILL",
    "SIGFPE",
    "SIGPIPE",
    "SIGQUIT",
    "EXECFAIL",
];

/// Statuses that are failures.
const FAILING: [&str; 11] = [
    "FAIL",
    "TIMEOUT",
    "LEAK-FAIL",
    "EXECFAIL",
    "SIGABRT",
    "SIGSEGV",
    "SIGKILL",
    "SIGTERM",
    "SIGBUS",
    "SIGILL",
    "SIGFPE",
];

/// A nextest status line: `FAIL [   0.084s] (1/3) fx tests::fails`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Status<'a> {
    word: &'a str,
    /// The test's name, without the progress counter; empty for a line with none.
    name: &'a str,
}

impl<'a> Status<'a> {
    fn read(line: &'a str) -> Option<Self> {
        let trimmed = line.trim_start();
        let (word, rest) = trimmed.split_once(' ')?;
        let rest = rest.trim_start();
        let word = STATUSES.iter().find(|known| **known == word)?;
        let after = rest.strip_prefix('[')?.split_once(']')?.1.trim_start();
        // the counter `(1/3)` or `(───)` is not part of the name
        let name = match after.strip_prefix('(') {
            Some(counted) => counted.split_once(')').map_or(after, |(_, name)| name),
            None => after,
        };
        Some(Self { word, name: name.trim() })
    }

    fn fails(&self) -> bool {
        FAILING.contains(&self.word)
    }
}

impl Tests {
    pub(super) fn of(log: &str) -> Option<Self> {
        let counts = log.lines().rev().find_map(|line| line.trim_start().strip_prefix("Summary [")?.split_once(']'))?.1;
        let mut tests = Self::default();
        tests.count(counts);
        tests.collect(log);
        Some(tests)
    }

    /// Reads `3 tests run: 0 passed, 1 failed, 2 timed out, 0 skipped`.
    fn count(&mut self, text: &str) {
        let words: Vec<&str> =
            text.split(|c: char| c.is_whitespace() || c == ',' || c == ':').filter(|w| !w.is_empty()).collect();
        for (at, word) in words.iter().enumerate() {
            // a cancelled run says `85/159 tests run`: the first is how many ran
            let Some(number) = word.split('/').next().and_then(|run| run.parse::<u32>().ok()) else { continue };
            let next = words.get(at + 1).copied();
            let after = words.get(at + 2).copied();
            match (next, after) {
                (Some("tests" | "test"), Some("run")) => self.run = number,
                (Some("passed"), _) => self.passed = number,
                (Some("failed"), _) => self.failed = number,
                (Some("timed"), Some("out")) => self.timed_out = number,
                (Some("skipped"), _) => self.skipped = number,
                _ => {}
            }
        }
    }

    /// Every failing test once, with the first output it was given.
    fn collect(&mut self, log: &str) {
        let mut open: Option<usize> = None;
        for line in log.lines() {
            if let Some(status) = Status::read(line) {
                open = status.fails().then(|| self.open(&status));
            } else if line.trim_start().starts_with("Cancelling")
                || line.starts_with("────")
                || line.trim_start().starts_with("Summary [")
            {
                open = None;
            } else if let Some(failure) = open.and_then(|at| self.failures.get_mut(at)) {
                failure.add(line);
            }
        }
        for failure in &mut self.failures {
            failure.settle();
        }
    }

    /// The index of the failure for `status`, added if new; a repeat does not take output again.
    fn open(&mut self, status: &Status<'_>) -> usize {
        match self.failures.iter().position(|failure| failure.name == status.name) {
            Some(at) if self.failures.get(at).is_some_and(|failure| failure.output.is_empty() && failure.cut == 0) => {
                at
            }
            Some(_) => usize::MAX,
            None => {
                self.failures.push(Failure {
                    status: status.word.to_owned(),
                    name: status.name.to_owned(),
                    output: Vec::new(),
                    cut: 0,
                });
                self.failures.len() - 1
            }
        }
    }
}

impl Failure {
    fn add(&mut self, line: &str) {
        if self.output.len() < OUTPUT_LINES {
            self.output.push(line.strip_prefix("    ").unwrap_or(line).trim_end().to_owned());
        } else {
            self.cut += 1;
        }
    }

    /// Drops the blank lines at both ends.
    fn settle(&mut self) {
        while self.output.last().is_some_and(String::is_empty) {
            self.output.pop();
        }
        let lead = self.output.iter().take_while(|line| line.is_empty()).count();
        self.output.drain(..lead);
    }
}

impl fmt::Display for Tests {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            out,
            "tests: {} run, {} passed, {} failed, {} timed out, {} skipped",
            self.run, self.passed, self.failed, self.timed_out, self.skipped
        )?;
        for failure in &self.failures {
            writeln!(out, "{} {}", failure.status, failure.name)?;
            failure.output.iter().try_for_each(|line| writeln!(out, "    {line}"))?;
            if failure.cut > 0 {
                writeln!(out, "    ... {} more lines in the log", failure.cut)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_line_reads_with_or_without_a_counter() {
        let status = Status::read("        FAIL [   0.084s] (1/3) fx tests::fails").unwrap();
        assert_eq!((status.word, status.name), ("FAIL", "fx tests::fails"));
        let cut = Status::read(" TERMINATING [>  0.100s] (───) fx tests::hangs").unwrap();
        assert_eq!((cut.word, cut.name), ("TERMINATING", "fx tests::hangs"));
        assert!(Status::read("    some stdout").is_none());
        assert!(Status::read("FAIL: not a status").is_none());
    }
}
