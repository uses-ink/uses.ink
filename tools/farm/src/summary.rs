//! A job's log, summarised: the test counts and every failing test with its output, the compiler
//! diagnostics, the gate's `check:` lines. The summary says what went wrong in a screen; the full
//! log stays on disk. It reads what nextest, cargo and `cargo xtask check` print with colour off
//! (`CARGO_TERM_COLOR=never`).

use std::fmt;

use serde::Serialize;

/// The most lines of one failing test's output that are shown.
pub(crate) const OUTPUT_LINES: usize = 60;
/// The most diagnostics that are shown.
pub(crate) const DIAGNOSTICS: usize = 10;

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
struct Status<'a> {
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
    fn of(log: &str) -> Option<Self> {
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
            let Some(number) = word.parse::<u32>().ok() else { continue };
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

/// What the compiler said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Diagnostics {
    pub(crate) errors: u32,
    pub(crate) warnings: u32,
    /// The first [`DIAGNOSTICS`], errors first, in short form.
    pub(crate) first: Vec<String>,
}

/// One diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Diagnostic {
    error: bool,
    code: Option<String>,
    at: String,
    message: String,
}

impl Diagnostic {
    /// Reads `error[E0425]: cannot find value` / `warning: unused variable`.
    fn head(line: &str) -> Option<(bool, Option<String>, &str)> {
        let (error, rest) = match (line.strip_prefix("error"), line.strip_prefix("warning")) {
            (Some(rest), _) => (true, rest),
            (_, Some(rest)) => (false, rest),
            _ => return None,
        };
        let (code, message) = match rest.strip_prefix('[') {
            Some(inner) => {
                let (code, tail) = inner.split_once("]: ")?;
                (Some(code.to_owned()), tail)
            }
            None => (None, rest.strip_prefix(": ")?),
        };
        let summary = [
            "could not compile",
            "aborting due to",
            "test run failed",
            "build failed",
            "check:",
            "layout problems",
            "stale generated",
        ];
        if summary.iter().any(|start| message.starts_with(start)) || message.contains(" generated ") {
            return None;
        }
        Some((error, code, message))
    }

    /// Reads `--> src/lib.rs:10:23`.
    fn place(line: &str) -> Option<&str> {
        line.trim_start().strip_prefix("--> ")
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = if self.error { "error" } else { "warning" };
        match &self.code {
            Some(code) => write!(out, "{level}[{code}] {}: {}", self.at, self.message),
            None => write!(out, "{level} {}: {}", self.at, self.message),
        }
    }
}

impl Diagnostics {
    fn of(log: &str) -> Self {
        let mut found: Vec<Diagnostic> = Vec::new();
        let lines: Vec<&str> = log.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            let Some((error, code, message)) = Diagnostic::head(line) else { continue };
            let place = lines.iter().skip(at + 1).take(3).find_map(|next| Diagnostic::place(next));
            let linking = message.starts_with("linking with") || message.starts_with("linker");
            let Some(place) = place.or(linking.then_some("link")) else { continue };
            let diagnostic = Diagnostic { error, code, at: place.to_owned(), message: message.to_owned() };
            if !found.contains(&diagnostic) {
                found.push(diagnostic);
            }
        }
        let errors = found.iter().filter(|d| d.error).count();
        found.sort_by_key(|d| !d.error);
        Self {
            errors: u32::try_from(errors).unwrap_or(u32::MAX),
            warnings: u32::try_from(found.len() - errors).unwrap_or(u32::MAX),
            first: found.iter().take(DIAGNOSTICS).map(ToString::to_string).collect(),
        }
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
        let Diagnostics { errors, warnings, first } = &self.diagnostics;
        if *errors + *warnings > 0 {
            writeln!(out, "compiler: {errors} errors, {warnings} warnings")?;
            first.iter().try_for_each(|line| writeln!(out, "  {line}"))?;
            let shown = u32::try_from(first.len()).unwrap_or(u32::MAX);
            if errors + warnings > shown {
                writeln!(out, "  ... {} more in the log", errors + warnings - shown)?;
            }
        }
        Ok(())
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
    use std::fmt::Write as _;

    use super::*;

    /// Real output, `CARGO_TERM_COLOR=never`.
    const NEXTEST: &str = r"   Compiling fx v0.1.0 (/w/fx)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.86s
────────────
 Nextest run ID 00000000-0000-0000-0000-000000000000 with nextest profile: default
    Starting 3 tests across 1 binary
        FAIL [   0.084s] (1/3) fx tests::fails
  stdout ───

    running 1 test
    some stdout
    test tests::fails ... FAILED

    failures:

    failures:
        tests::fails

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

  stderr ───

    thread 'tests::fails' (1779071) panicked at src/lib.rs:6:51:
    assertion `left == right` failed: math is hard
      left: 3
     right: 4
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

  Cancelling due to test failure: 2 tests still running
 TERMINATING [>  0.100s] (───) fx tests::hangs
 TERMINATING [>  0.100s] (───) fx tests::passes
     TIMEOUT [   0.119s] (2/3) fx tests::hangs
  stdout ───

    running 1 test

    (test timed out)

     TIMEOUT [   0.129s] (3/3) fx tests::passes
  stdout ───

    running 1 test

    (test timed out)

────────────
     Summary [   0.149s] 3 tests run: 0 passed, 1 failed, 2 timed out, 0 skipped
        FAIL [   0.084s] (1/3) fx tests::fails
     TIMEOUT [   0.119s] (2/3) fx tests::hangs
     TIMEOUT [   0.129s] (3/3) fx tests::passes
error: test run failed
";

    /// Real output, `CARGO_TERM_COLOR=never`.
    const CLIPPY: &str = r"    Checking fx v0.1.0 (/w/fx)
warning: unused variable: `unused`
 --> src/lib.rs:9:21
  |
9 | pub fn warn() { let unused = 1; let v = vec![1]; let _ = v.len() == 0; }
  |                     ^^^^^^ help: if this is intentional, prefix it with an underscore: `_unused`
  |
  = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: length comparison to zero
 --> src/lib.rs:9:58
  |
9 | pub fn warn() { let unused = 1; let v = vec![1]; let _ = v.len() == 0; }
  |                                                          ^^^^^^^^^^^^ help: using `is_empty` is clearer and more explicit: `v.is_empty()`
  |
  = help: for further information visit https://clippy.rust-lang.org/main/index.html#len_zero
  = note: `#[warn(clippy::len_zero)]` on by default

warning: useless use of `vec!`
 --> src/lib.rs:9:41
  |
9 | pub fn warn() { let unused = 1; let v = vec![1]; let _ = v.len() == 0; }
  |                                         ^^^^^^^ help: you can use an array directly: `[1]`
  |
  = help: for further information visit https://clippy.rust-lang.org/main/index.html#useless_vec
  = note: `#[warn(clippy::useless_vec)]` on by default

warning: `fx` (lib) generated 3 warnings (run `cargo clippy --fix --lib -p fx -- ` to apply 3 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s
";

    /// Real output, `CARGO_TERM_COLOR=never`.
    const BUILD: &str = r"   Compiling fx v0.1.0 (/w/fx)
error[E0425]: cannot find value `missing` in this scope
  --> src/lib.rs:10:23
   |
10 | pub fn bad() -> u32 { missing }
   |                       ^^^^^^^ not found in this scope

warning: unused variable: `unused`
 --> src/lib.rs:9:21
  |
9 | pub fn warn() { let unused = 1; let v = vec![1]; let _ = v.len() == 0; }
  |                     ^^^^^^ help: if this is intentional, prefix it with an underscore: `_unused`
  |
  = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

For more information about this error, try `rustc --explain E0425`.
warning: `fx` (lib) generated 1 warning
error: could not compile `fx` (lib) due to 1 previous error; 1 warning emitted
";

    const GATE: &str = "check: cargo --locked fmt --all --check
check: cargo clippy --locked --workspace --all-targets -- -D warnings
error: check: `cargo clippy --locked --workspace --all-targets -- -D warnings` failed
  caused by: it ended with exit status: 101
check: layout
error: layout problems:
  xtask/src/x.rs: function `a_b_c` has three words
";

    #[test]
    fn nextest_failures_and_timeouts_are_named_once_with_their_output() {
        let summary = Summary::of(NEXTEST);
        let tests = summary.tests.unwrap();
        assert_eq!((tests.run, tests.passed, tests.failed, tests.timed_out, tests.skipped), (3, 0, 1, 2, 0));
        let names: Vec<&str> = tests.failures.iter().map(|failure| failure.name.as_str()).collect();
        assert_eq!(names, ["fx tests::fails", "fx tests::hangs", "fx tests::passes"]);
        let fails = tests.failures.first().unwrap();
        assert_eq!(fails.status, "FAIL");
        assert!(fails.output.iter().any(|line| line == "some stdout"), "{:?}", fails.output);
        assert!(fails.output.iter().any(|line| line.contains("math is hard")));
        assert!(fails.output.iter().any(|line| line.contains("left: 3")));
        let hangs = tests.failures.get(1).unwrap();
        assert_eq!(hangs.status, "TIMEOUT");
        assert!(hangs.output.iter().any(|line| line.contains("(test timed out)")));
        assert!(!fails.output.iter().any(|line| line.contains("Cancelling")));
    }

    #[test]
    fn a_long_failure_is_cut_at_sixty_lines_and_says_how_many() {
        let mut log = String::from("        FAIL [   0.1s] (1/1) x t\n  stdout ---\n");
        for n in 0..100 {
            writeln!(log, "    line {n}").unwrap();
        }
        log.push_str("     Summary [   0.2s] 1 tests run: 0 passed, 1 failed, 0 skipped\n");
        let tests = Summary::of(&log).tests.unwrap();
        let failure = tests.failures.first().unwrap();
        assert_eq!(failure.output.len(), OUTPUT_LINES);
        assert_eq!(failure.cut, 100 + 1 - OUTPUT_LINES);
    }

    #[test]
    fn a_passing_run_has_counts_and_no_failures() {
        let log = "    Starting 2 tests across 1 binary\n        PASS [   0.01s] (1/2) x a\n        PASS [   0.01s] (2/2) x b\n     Summary [   0.02s] 2 tests run: 2 passed, 0 skipped\n";
        let tests = Summary::of(log).tests.unwrap();
        assert_eq!((tests.run, tests.passed, tests.failed), (2, 2, 0));
        assert_eq!(tests.failures, Vec::new());
        assert_eq!(Summary::of("nothing here\n"), Summary::default());
    }

    #[test]
    fn clippy_warnings_are_counted_and_shown_short() {
        let diagnostics = Summary::of(CLIPPY).diagnostics;
        assert_eq!((diagnostics.errors, diagnostics.warnings), (0, 3));
        assert_eq!(
            diagnostics.first.first().map(String::as_str),
            Some("warning src/lib.rs:9:21: unused variable: `unused`")
        );
        assert!(diagnostics.first.iter().any(|line| line.contains("length comparison to zero")));
    }

    #[test]
    fn a_compile_error_counts_once_even_when_a_warning_repeats_and_comes_first() {
        let diagnostics = Summary::of(BUILD).diagnostics;
        assert_eq!((diagnostics.errors, diagnostics.warnings), (1, 1));
        assert_eq!(
            diagnostics.first.first().map(String::as_str),
            Some("error[E0425] src/lib.rs:10:23: cannot find value `missing` in this scope")
        );
    }

    #[test]
    fn more_than_ten_diagnostics_show_ten_and_the_rest_as_a_count() {
        let mut log = String::new();
        for n in 0..14 {
            writeln!(log, "warning: unused variable: `v{n}`\n --> src/lib.rs:{n}:1\n").unwrap();
        }
        let summary = Summary::of(&log);
        assert_eq!(summary.diagnostics.first.len(), DIAGNOSTICS);
        assert!(summary.to_string().contains("... 4 more in the log"));
    }

    #[test]
    fn gate_lines_and_error_chains_are_kept() {
        let summary = Summary::of(GATE);
        assert_eq!(summary.checks.len(), 7);
        assert!(summary.checks.iter().any(|line| line.starts_with("  caused by: it ended")));
        assert!(summary.checks.iter().any(|line| line.contains("function `a_b_c`")));
        assert_eq!(summary.diagnostics, Diagnostics::default());
    }

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
