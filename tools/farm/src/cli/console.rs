//! What the farm says to the person, and the one place it is written: notes and progress on
//! stderr, results on stdout, the exact commands under `--verbose`. A write that fails is
//! remembered, so the run can end as failed instead of silent.

use std::fmt;
use std::io::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The terminal, shared by every job running at once.
#[derive(Debug, Default)]
pub(crate) struct Console {
    verbose: bool,
    /// Lines are written whole: one job's progress never lands inside another's.
    lock: Mutex<()>,
    broken: AtomicBool,
}

impl Console {
    pub(crate) fn new(verbose: bool) -> Self {
        Self { verbose, ..Self::default() }
    }

    /// A line of progress or a notice, on stderr.
    pub(crate) fn note(&self, text: impl fmt::Display) {
        let _held = self.lock.lock();
        self.settle(&writeln!(std::io::stderr().lock(), "farm: {text}"));
    }

    /// A line as it is, on stderr: an error with its causes.
    pub(crate) fn fail(&self, text: impl fmt::Display) {
        let _held = self.lock.lock();
        self.settle(&writeln!(std::io::stderr().lock(), "{text}"));
    }

    /// A line of a result, on stdout.
    pub(crate) fn out(&self, text: impl fmt::Display) {
        let _held = self.lock.lock();
        self.settle(&writeln!(std::io::stdout().lock(), "{text}"));
    }

    /// A command about to run, when asked for with `--verbose`.
    pub(crate) fn show(&self, text: impl fmt::Display) {
        if self.verbose {
            let _held = self.lock.lock();
            self.settle(&writeln!(std::io::stderr().lock(), "farm: $ {text}"));
        }
    }

    /// Whether a write has failed.
    pub(crate) fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Relaxed)
    }

    fn settle(&self, written: &std::io::Result<()>) {
        if written.is_err() {
            self.broken.store(true, Ordering::Relaxed);
        }
    }
}
