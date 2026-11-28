//! The stage deciding `file.ignore`: files under the root that are never part of the site.

use std::collections::BTreeSet;

use model::code::{Ignored, Layout};
use model::decision::{File, Site};
use model::{Announcement, ContentPath, Evidence, FileFacts, Notice};

use crate::Pins;
use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::About;
use crate::interpret::decide::precedence;
use crate::interpret::journal::Journal;
use crate::interpret::pipeline::stage::Stage;
use crate::interpret::pipeline::tree::{InRoot, Scope};
use crate::interpret::pipeline::vault::Templates;

/// What `file.ignore` is decided on: the file, and how the site is arranged, as a vault keeps
/// its templates in a folder of their own.
#[derive(Debug)]
pub(crate) struct Ground<'a> {
    pub(crate) file: &'a InRoot<'a>,
    pub(crate) layout: Option<model::Layout>,
    /// Where a vault keeps its templates.
    pub(crate) templates: &'a Templates,
}

impl Filed for Ground<'_> {
    fn path(&self) -> &ContentPath {
        self.file.path()
    }

    fn relative(&self) -> &ContentPath {
        self.file.relative()
    }

    fn facts(&self) -> Option<&FileFacts> {
        self.file.facts()
    }
}

/// Decides which files under the root are kept.
#[derive(Debug)]
pub(crate) struct Files<'a, P> {
    pub(crate) pins: &'a P,
    pub(crate) templates: &'a Templates,
}

impl<'s, P: Pins> Stage<&Scope<'s>> for Files<'_, P> {
    /// The files kept, in path order.
    type Out = Vec<InRoot<'s>>;

    fn run(&self, scope: &Scope<'s>, journal: &mut Journal) -> Vec<InRoot<'s>> {
        let ladder = precedence::Ignored::default();
        let mut kept = Vec::new();
        let mut announced = Announced::default();
        let layout = journal.value::<Layout>(&Site).copied();
        for file in &scope.inside {
            let subject = File(file.entry.path.clone());
            let ground = Ground { file, layout, templates: self.templates };
            match ladder.record(About { subject: &subject, ground: &ground }, self.pins, journal) {
                Some(false) => kept.push(file.clone()),
                Some(true) => {
                    let evidence = journal.decision::<Ignored>(&subject).map(|decision| decision.evidence.clone());
                    announced.announce(evidence, &subject, journal);
                }
                None => {}
            }
        }
        kept
    }
}

/// The folders and files already announced as ignored by inference, so a hidden folder of a
/// thousand files is announced once.
#[derive(Debug, Default)]
struct Announced(BTreeSet<ContentPath>);

impl Announced {
    /// `file.ignore.inferred` for `file`, ignored on `evidence`, when a folder or a name ignored
    /// it rather than the author: once per folder or file, on the first file it ignored.
    fn announce(&mut self, evidence: Option<Evidence>, file: &File, journal: &mut Journal) {
        let Some(Evidence::Marker { path } | Evidence::Filename { path }) = evidence else { return };
        if self.0.insert(path.clone()) {
            journal.notice(Notice::new(Announcement::FileIgnoreInferred { path }).on(file.clone().into()));
        }
    }
}
