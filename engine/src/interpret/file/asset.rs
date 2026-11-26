//! `page.publish` for files no format reads: a plain file is published only when a published
//! page uses it (decision 0017).

use std::collections::BTreeMap;

use model::{ContentPath, DefaultReason, Evidence};

use crate::interpret::file::scope::InRoot;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Pin, then use by a published page (the evidence names the first page using it), then not
/// published. Returns the files published, in path order.
pub(crate) fn decide<'a>(
    files: &[InRoot<'a>],
    used: &BTreeMap<ContentPath, ContentPath>,
    pins: &impl Pins,
    record: &mut Record,
) -> Vec<InRoot<'a>> {
    let mut published = Vec::new();
    for file in files {
        let by_page = used.get(file.path()).map(|page| (true, Evidence::Marker { path: page.clone() }));
        let publish = Ladder::<code::Publish>::new(file.subject(), pins, record).rung(by_page).or_default(
            false,
            DefaultReason::NothingMatched,
            record,
        );
        if publish {
            published.push(file.clone());
        }
    }
    published
}
