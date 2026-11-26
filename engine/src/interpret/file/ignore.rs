//! The stage deciding `file.ignore`: files under the root that are never part of the site.

use std::collections::BTreeSet;

use model::code::Ignored;
use model::decision::File;
use model::{Announcement, ContentPath, Evidence, Notice};

use crate::Pins;
use crate::interpret::decide::ladder::About;
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
        let mut said = Said::default();
        for file in &scope.inside {
            let subject = File(file.entry.path.clone());
            match ladder.record(About { subject: &subject, ground: file }, self.pins, journal) {
                Some(false) => kept.push(file.clone()),
                Some(true) => {
                    let evidence = journal.decision::<Ignored>(&subject).map(|decision| decision.evidence.clone());
                    said.announce(evidence, &subject, journal);
                }
                None => {}
            }
        }
        kept
    }
}

/// The folders and files already announced as ignored by inference, so a hidden folder of a
/// thousand files is said once.
#[derive(Debug, Default)]
struct Said(BTreeSet<ContentPath>);

impl Said {
    /// `file.ignore.inferred` for `file`, ignored on `evidence`, when a folder or a name ignored
    /// it rather than the author: once per folder or file, on the first file it ignored.
    fn announce(&mut self, evidence: Option<Evidence>, file: &File, journal: &mut Journal) {
        let Some(Evidence::Marker { path } | Evidence::Filename { path }) = evidence else { return };
        if self.0.insert(path.clone()) {
            journal.notice(Notice::new(Announcement::FileIgnoreInferred { path }).on(file.clone().into()));
        }
    }
}
