//! The stage deciding `site.root`, and splitting the snapshot there.

use model::Snapshot;
use model::decision::Site;

use crate::interpret::decide::ladder::About;
use crate::interpret::decide::precedence;
use crate::interpret::journal::Journal;
use crate::interpret::pipeline::stage::Stage;
use crate::interpret::pipeline::tree::{Scope, Tree};
use crate::{Formats, Pins};

/// Decides where the site's content starts, and gives the files under it.
#[derive(Debug)]
pub(crate) struct Root<'a, F, P> {
    pub(crate) formats: &'a F,
    pub(crate) pins: &'a P,
}

impl<'s, F: Formats, P: Pins> Stage<&'s Snapshot> for Root<'_, F, P> {
    type Out = Scope<'s>;

    fn run(&self, snapshot: &'s Snapshot, journal: &mut Journal) -> Scope<'s> {
        let readable =
            snapshot.tree().iter().map(|entry| &entry.path).filter(|path| self.formats.handles(path)).collect();
        let tree = Tree { snapshot, readable };
        let root = precedence::Root::default().record(About { subject: &Site, ground: &tree }, self.pins, journal);
        // A root that could not be decided is already a recorded error, which stops the build;
        // the top keeps the later stages reporting.
        Scope::new(snapshot, root.unwrap_or(model::Root::Top))
    }
}
