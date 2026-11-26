//! Names a body gives its parts: heading anchors and footnote names. Both are the slug of what
//! the author wrote; a text with no letters or digits gets a generated name, and says so.

use std::collections::BTreeSet;

use model::{Anchor, Announcement, FootnoteName, Hash, Slug, Span};

use crate::error::Report;

/// The anchor of a heading with no letters or digits.
const SECTION: &str = "section";

/// The hexadecimal digits of a hash a generated footnote name keeps.
const DIGITS: usize = 8;

/// A kind of name [`Names`] gives.
pub(crate) trait Name: Sized {
    /// The name of a text with no letters or digits.
    fn fallback(given: &str) -> Option<Slug>;

    /// The name for `slug`, given the anchors already taken.
    fn claim(slug: Slug, taken: &mut BTreeSet<Slug>) -> Self;

    /// The notice that this name was generated for `given`.
    fn generated(&self, given: &str) -> Announcement;
}

/// The names given so far in one body.
#[derive(Debug, Default)]
pub(crate) struct Names {
    /// Anchors given so far, so a repeated heading gets `-1`, `-2`.
    taken: BTreeSet<Slug>,
}

impl Names {
    /// The name of kind `N` for the text `given`. A generated name is announced at `at`, when
    /// given: a footnote's definition announces it, its references do not.
    pub(crate) fn name<N: Name>(&mut self, given: &str, at: Option<Span>, report: &mut Report) -> Option<N> {
        let (slug, generated) = match Slug::from_text(given) {
            Ok(slug) => (slug, false),
            Err(_) => (N::fallback(given)?, true),
        };
        let name = N::claim(slug, &mut self.taken);
        if generated && let Some(span) = at {
            report.announce(name.generated(given), span);
        }
        Some(name)
    }
}

impl Name for Anchor {
    /// `section`.
    fn fallback(_: &str) -> Option<Slug> {
        Slug::from_text(SECTION).ok()
    }

    /// `slug`, or the first of `slug-1`, `slug-2` not yet taken.
    fn claim(slug: Slug, taken: &mut BTreeSet<Slug>) -> Self {
        let mut candidate = slug.clone();
        let mut count = 0_u64;
        while taken.contains(&candidate) {
            count += 1;
            match Slug::parse(format!("{slug}-{count}")) {
                Ok(next) => candidate = next,
                Err(_) => break,
            }
        }
        taken.insert(candidate.clone());
        Self::new(candidate)
    }

    fn generated(&self, _: &str) -> Announcement {
        Announcement::HeadingAnchorGenerated { anchor: self.to_string() }
    }
}

impl Name for FootnoteName {
    /// `note-` and the start of the text's hash, the same for a reference and its definition.
    fn fallback(given: &str) -> Option<Slug> {
        let digest = Hash::of(given.as_bytes()).to_string();
        Slug::parse(format!("note-{}", digest.get(..DIGITS)?)).ok()
    }

    /// The slug itself: a reference and its definition share their name.
    fn claim(slug: Slug, _: &mut BTreeSet<Slug>) -> Self {
        slug.into()
    }

    fn generated(&self, given: &str) -> Announcement {
        Announcement::FootnoteNameGenerated { name: self.to_string(), given: given.to_owned() }
    }
}
