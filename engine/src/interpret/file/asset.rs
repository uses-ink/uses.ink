//! The stage deciding `page.publish` for files no format reads: a plain file is published only
//! when a published page uses it (decision 0017).

use std::collections::BTreeMap;

use model::code::Publish;
use model::decision::File;
use model::{ContentPath, Evidence, FileFacts};

use crate::Pins;
use crate::interpret::decide::Filed;
use crate::interpret::decide::ladder::{About, Case, Testimony, Witness};
use crate::interpret::decide::precedence;
use crate::interpret::decide::trace::{Named, Witnessed};
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::InRoot;

/// Each plain file a published page shows, with the first page that does.
pub(crate) type Uses = BTreeMap<ContentPath, ContentPath>;

/// What `page.publish` of a plain file is decided on.
#[derive(Debug)]
pub(crate) struct Ground<'a> {
    pub(crate) file: &'a InRoot<'a>,
    /// The first published page that uses it, if one does.
    pub(crate) user: Option<&'a ContentPath>,
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

/// A published page using the file, evidence `Marker` naming the first page that does.
#[derive(Debug, Default)]
pub(crate) struct Used;

impl Witness<Publish, Ground<'_>> for Used {
    fn testify(&self, case: &Case<'_, Publish, Ground<'_>>, testimony: &mut Testimony<bool>) {
        if let Some(page) = case.ground.user {
            testimony.value(true, Evidence::Marker { path: page.clone() });
        }
    }
}

/// Decides which plain files are published.
#[derive(Debug)]
pub(crate) struct Assets<'a, P> {
    pub(crate) pins: &'a P,
    pub(crate) used: &'a Uses,
}

impl<'s, P: Pins> Stage<&[InRoot<'s>]> for Assets<'_, P> {
    /// The files published, in path order.
    type Out = Vec<InRoot<'s>>;

    fn run(&self, files: &[InRoot<'s>], journal: &mut Journal) -> Vec<InRoot<'s>> {
        let ladder = precedence::Plain::default();
        let mut published = Vec::new();
        for file in files {
            let ground = Ground { file, user: self.used.get(&file.entry.path) };
            let subject = File(file.entry.path.clone());
            if ladder.record(About { subject: &subject, ground: &ground }, self.pins, journal) == Some(true) {
                published.push(file.clone());
            }
        }
        published
    }
}

impl Named for Used {
    const NAME: Witnessed = Witnessed::Used;
}
