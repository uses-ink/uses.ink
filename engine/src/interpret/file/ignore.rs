//! The stage deciding `file.ignore`: files under the root that are never part of the site.

use model::decision::File;

use crate::Pins;
use crate::interpret::decide::precedence;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::{InRoot, Scope};

/// Decides which files under the root are kept.
#[derive(Debug)]
pub(crate) struct Files<'a, P> {
    pub(crate) pins: &'a P,
}

impl<'s, P: Pins> Stage<&Scope<'s>> for Files<'_, P> {
    /// The files kept, in path order.
    type Out = Vec<InRoot<'s>>;

    fn run(&self, scope: &Scope<'s>, journal: &mut Journal) -> Vec<InRoot<'s>> {
        let ladder = precedence::Ignored::default();
        let mut kept = Vec::new();
        for file in &scope.inside {
            let subject = File(file.entry.path.clone());
            if ladder.record(&subject, file, self.pins, journal) == Some(false) {
                kept.push(file.clone());
            }
        }
        kept
    }
}
