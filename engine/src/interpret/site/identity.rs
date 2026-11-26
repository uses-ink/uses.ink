//! `site.title`, `site.description`, `site.authors`, `site.language`: what the site says about
//! itself, mostly taken from its home page.

use std::fmt;

use model::{ContentPath, DefaultReason, Evidence, Language, Root, Subject};

use crate::error::Broken;
use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// The language a site is taken to be written in when nothing says.
const LANGUAGE: &str = "en";

/// The title of a site nothing names.
#[derive(Clone, Copy, Debug)]
struct Untitled;

impl fmt::Display for Untitled {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Untitled site")
    }
}

/// Decides all four, after `site.home`. `home` is the home page's file, if there is one.
pub(crate) fn decide(home: Option<&Source>, root: &Root, pins: &impl Pins, record: &mut Record) {
    title(home, root, pins, record);
    let summary = home
        .and_then(|home| taken::<code::Summary>(home, record))
        .and_then(|(summary, evidence)| summary.map(|summary| (Some(summary), evidence)));
    Ladder::<code::Description>::new(Subject::Site, pins, record).rung(summary).or_default(
        None,
        DefaultReason::NothingMatched,
        record,
    );
    Ladder::<code::Authors>::new(Subject::Site, pins, record).or_default(
        Vec::new(),
        DefaultReason::NothingMatched,
        record,
    );
    language(home, pins, record);
}

/// Pin, then the home page's title, then the root folder's name, then [`Untitled`].
fn title(home: Option<&Source>, root: &Root, pins: &impl Pins, record: &mut Record) {
    let from_home = home.and_then(|home| taken::<code::Title>(home, record));
    let folder = match root {
        Root::Folder(folder) => Some((folder.file_name().to_owned(), Evidence::Marker { path: folder.clone() })),
        _ => None,
    };
    Ladder::<code::SiteTitle>::new(Subject::Site, pins, record).rung(from_home).rung(folder).or_default(
        Untitled.to_string(),
        DefaultReason::NothingMatched,
        record,
    );
}

/// Pin, then the home page's frontmatter `lang`, then [`LANGUAGE`].
fn language(home: Option<&Source>, pins: &impl Pins, record: &mut Record) {
    let front = home.and_then(|home| {
        let language = home.read.front.language.as_ref()?;
        Some((language.value.clone(), home.frontmatter(language.span)))
    });
    let ladder = Ladder::<code::Language>::new(Subject::Site, pins, record).rung(front);
    match LANGUAGE.parse::<Language>() {
        Ok(default) => {
            ladder.or_default(default, DefaultReason::NothingMatched, record);
        }
        Err(source) => record.broken(Broken::Language { written: LANGUAGE, source }),
    }
}

/// The home page's decided value of `C`, with the evidence that decided it there.
fn taken<C: crate::interpret::ladder::Code>(home: &Source, record: &Record) -> Option<(C::Value, Evidence)> {
    let subject = Subject::File(ContentPath::clone(&home.path));
    let decision = record.decision::<C>(&subject)?;
    Some((C::unwrap(&decision.decided)?.clone(), decision.evidence.clone()))
}
