//! A stage of interpreting: one step from what is known to what follows, recording everything it
//! decides and says into the journal.

use crate::interpret::journal::Journal;

/// One step of interpreting, from `I` to [`Stage::Out`]. Every decision, diagnostic and notice
/// goes into `journal`; nothing is returned on the side.
pub(crate) trait Stage<I> {
    /// What the step gives the next.
    type Out;

    /// Runs the step.
    fn run(&self, input: I, journal: &mut Journal) -> Self::Out;
}
