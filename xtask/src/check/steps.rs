//! A step of the gate, and lists of them.

use std::fmt;
use std::path::Path;

use super::Gate;
use crate::report::Report;

/// One step of the gate.
pub(crate) trait Step: fmt::Display {
    /// Runs it from the workspace at `root`, its lines and failures into `report`.
    fn run(&self, root: &Path, report: &mut Report);
}

/// A list of steps, run in order. Implemented for tuples of [`Step`]s.
pub(crate) trait Steps {
    /// Runs each, announced as it starts.
    fn run(&self, gate: &mut Gate);
}

tuples! { Steps: Step, fn run(&self, gate: &mut Gate) { |step| gate.step(step) } }
