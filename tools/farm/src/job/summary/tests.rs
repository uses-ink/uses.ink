//! The summaries of real cargo and nextest output.

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
        Some("warning[unused_variables] src/lib.rs:9:21: unused variable: `unused`")
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

/// Real output of a clippy failure under `-D warnings`, as the farm's logs hold it.
const DENIED: &str = r"warning: unused workspace dependency `web`
  --> Cargo.toml:110:1
   |
   = note: `cargo::unused_workspace_dependencies` is set to `warn` by default
warning: workspace (manifest) generated 1 warning
    Checking uses-ink-engine v0.0.0 (/home/cstef/farm/uses.ink/trace/engine)
error: this function has too many lines (45/40)
   --> engine/src/interpret.rs:107:5
    |
107 |     fn run(&self, snapshot: &Snapshot, mut journal: Journal) -> Understood {
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.100.0/index.html#too_many_lines
    = note: `-D clippy::too-many-lines` implied by `-D warnings`
    = help: to override `-D warnings` add `#[allow(clippy::too_many_lines)]`

error: consider adding a `;` to the last statement for consistent formatting
  --> edge/api/src/api.rs:73:17
   |
73 |                 trace::warn!(name: trace::event::API_AUTH)
   |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: add a `;` here
   |
   = note: `-D clippy::semicolon-if-nothing-returned` implied by `-D warnings`

error: could not compile `uses-ink-engine` (lib) due to 1 previous error
";

#[test]
fn a_clippy_failure_shows_its_first_error_with_file_line_and_lint_in_a_screen() {
    let summary = Summary::of(DENIED);
    let diagnostics = &summary.diagnostics;
    assert_eq!((diagnostics.errors, diagnostics.warnings), (2, 1));
    let shown = summary.to_string();
    let lines: Vec<&str> = shown.lines().collect();
    assert!(lines.len() <= 15, "{shown}");
    assert_eq!(
        lines.get(1).copied(),
        Some("  error[clippy::too-many-lines] engine/src/interpret.rs:107:5: this function has too many lines (45/40)")
    );
    assert!(shown.contains("107 |     fn run(&self"), "the code is shown: {shown}");
    assert!(!shown.contains("to override"), "the lint advice is noise: {shown}");
    assert!(shown.contains("error[clippy::semicolon-if-nothing-returned] edge/api/src/api.rs:73:17"), "{shown}");
    assert!(shown.contains("warning[cargo::unused_workspace_dependencies] Cargo.toml:110:1"), "{shown}");
    assert!(!shown.contains("more in the log"), "all three fit: {shown}");
}

#[test]
fn a_diagnostic_printed_twice_with_different_notes_counts_once() {
    let log =
        "warning: unused thing\n --> Cargo.toml:1:1\n  |\n  = note: a\nwarning: unused thing\n --> Cargo.toml:1:1\n";
    assert_eq!(Summary::of(log).diagnostics.warnings, 1);
}

#[test]
fn a_cancelled_run_counts_the_tests_that_ran() {
    let log = "     Summary [   1.068s] 85/159 tests run: 84 passed, 1 failed, 0 skipped\n";
    let tests = Summary::of(log).tests.unwrap();
    assert_eq!((tests.run, tests.passed, tests.failed), (85, 84, 1));
}

#[test]
fn gate_lines_and_error_chains_are_kept() {
    let summary = Summary::of(GATE);
    assert_eq!(summary.checks.len(), 7);
    assert!(summary.checks.iter().any(|line| line.starts_with("  caused by: it ended")));
    assert!(summary.checks.iter().any(|line| line.contains("function `a_b_c`")));
    assert_eq!(summary.diagnostics, Diagnostics::default());
}
