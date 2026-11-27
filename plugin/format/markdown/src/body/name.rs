//! Names a body gives its parts: heading anchors and footnote names. Both are the slug of what
//! the author wrote, unique in the body; a text with no letters or digits gets a generated name,
//! and a footnote whose slug another footnote took gets the next free one, and each says so.

use std::collections::{BTreeMap, BTreeSet};

use model::{Anchor, Announcement, FootnoteName, Hash, Slug, Span};

use crate::error::Report;

/// The anchor of a heading with no letters or digits.
const SECTION: &str = "section";

/// The hexadecimal digits of a hash a generated footnote name keeps.
const DIGITS: usize = 8;

/// What a text written again means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Repeat {
    /// Another thing of the same text: a heading written twice gets a second anchor.
    Another,
    /// The same thing: a footnote's references and its definition share one name.
    Same,
}

/// A kind of name [`Names`] gives.
pub(crate) trait Name: Clone + From<Slug> {
    /// What the same text written again means.
    const REPEAT: Repeat;

    /// The name of a text with no letters or digits.
    fn fallback(given: &str) -> Option<Slug>;

    /// The names of this kind given so far.
    fn claims(names: &mut Names) -> &mut Claims<Self>;

    /// The notice that this name was generated for `given`.
    fn generated(&self, given: &str) -> Announcement;

    /// The notice that `given`'s slug was taken, so it got this name instead, if that is worth
    /// saying: a repeated heading's `-1` is expected, a footnote's is not.
    fn renamed(&self, given: &str) -> Option<Announcement>;
}

/// How a name came from its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// The text's slug.
    Written,
    /// Generated, as the text has no letters or digits.
    Generated,
    /// The next free slug, as another text took the text's own.
    Renamed,
}

/// The names of one kind given so far in one body.
#[derive(Debug)]
pub(crate) struct Claims<N> {
    /// Every slug taken.
    taken: BTreeSet<Slug>,
    /// For a kind whose repeated text is the same thing: the name each text got, by the text
    /// as `CommonMark` matches labels, in lower case.
    given: BTreeMap<String, (N, Origin)>,
    /// For each slug claimed again, the last count its claim reached and the name it gave:
    /// every `slug-n` up to it is taken, and stays taken, so the next claim goes on from there
    /// instead of starting again at `slug-1`.
    counted: BTreeMap<Slug, (u64, Slug)>,
}

impl<N> Default for Claims<N> {
    fn default() -> Self {
        Self { taken: BTreeSet::new(), given: BTreeMap::new(), counted: BTreeMap::new() }
    }
}

impl<N> Claims<N> {
    /// `slug`, or the first of `slug-1`, `slug-2` not yet taken, now taken.
    fn claim(&mut self, slug: &Slug) -> Slug {
        let (mut count, mut candidate) = self.counted.get(slug).cloned().unwrap_or_else(|| (0, slug.clone()));
        while self.taken.contains(&candidate) {
            count += 1;
            match Slug::parse(format!("{slug}-{count}")) {
                Ok(next) => candidate = next,
                Err(_) => break,
            }
        }
        if count > 0 {
            self.counted.insert(slug.clone(), (count, candidate.clone()));
        }
        self.taken.insert(candidate.clone());
        candidate
    }
}

/// The names given so far in one body.
#[derive(Debug, Default)]
pub(crate) struct Names {
    anchors: Claims<Anchor>,
    notes: Claims<FootnoteName>,
}

impl Names {
    /// The name of kind `N` for the text `given`. A generated or renamed name is announced at
    /// `at`, when given: a footnote's definition announces it, its references do not.
    pub(crate) fn name<N: Name>(&mut self, given: &str, at: Option<Span>, report: &mut Report) -> Option<N> {
        let key = given.to_lowercase();
        let claims = N::claims(self);
        let known = claims.given.get(&key).filter(|_| N::REPEAT == Repeat::Same).cloned();
        let (name, origin) = if let Some(known) = known {
            known
        } else {
            let (slug, origin) = match Slug::from_text(given) {
                Ok(slug) => (slug, Origin::Written),
                Err(_) => (N::fallback(given)?, Origin::Generated),
            };
            let claimed = claims.claim(&slug);
            let origin = if claimed == slug || origin == Origin::Generated { origin } else { Origin::Renamed };
            let name = N::from(claimed);
            if N::REPEAT == Repeat::Same {
                claims.given.insert(key, (name.clone(), origin));
            }
            (name, origin)
        };
        let announcement = match origin {
            Origin::Written => None,
            Origin::Generated => Some(name.generated(given)),
            Origin::Renamed => name.renamed(given),
        };
        if let (Some(announcement), Some(span)) = (announcement, at) {
            report.announce(announcement, span);
        }
        Some(name)
    }
}

impl Name for Anchor {
    const REPEAT: Repeat = Repeat::Another;

    /// `section`.
    fn fallback(_: &str) -> Option<Slug> {
        Slug::from_text(SECTION).ok()
    }

    fn claims(names: &mut Names) -> &mut Claims<Self> {
        &mut names.anchors
    }

    fn generated(&self, _: &str) -> Announcement {
        Announcement::HeadingAnchorGenerated { anchor: self.to_string() }
    }

    /// Nothing: a repeated heading's `-1` is what readers expect.
    fn renamed(&self, _: &str) -> Option<Announcement> {
        None
    }
}

impl Name for FootnoteName {
    const REPEAT: Repeat = Repeat::Same;

    /// `note-` and the start of the text's hash, the same for a reference and its definition.
    fn fallback(given: &str) -> Option<Slug> {
        let digest = Hash::of(given.as_bytes()).to_string();
        Slug::parse(format!("note-{}", digest.get(..DIGITS)?)).ok()
    }

    fn claims(names: &mut Names) -> &mut Claims<Self> {
        &mut names.notes
    }

    fn generated(&self, given: &str) -> Announcement {
        Announcement::FootnoteNameGenerated { name: self.to_string(), given: given.to_owned() }
    }

    fn renamed(&self, given: &str) -> Option<Announcement> {
        Some(Announcement::FootnoteNameTaken { name: self.to_string(), given: given.to_owned() })
    }
}
