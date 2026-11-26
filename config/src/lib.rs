//! `ink.toml`, the one optional file at a source's root: pins of decisions, the theme choice and
//! which files are part of the site.
//!
//! **The decision code is the `ink.toml` path.** A sitewide code `site.home` is pinned by
//! `[site] home = ...`; a per-file code `page.title` on `posts/x.md` by
//! `[page."posts/x.md"] title = ...`, and `file.ignore` by `[file."x"] ignore = true`. The
//! table and key come from [`Code::NAME`], and a value is read and written by its type, so
//! nothing here lists a pin or a code by hand.
//!
//! ```toml
//! include = ["notes/2023.md"]
//! exclude = ["notes/private/**"]
//!
//! [theme]
//! use = "ink:theme-quiet"          # the theme plugin; omitted means the default theme
//! accent = "#2b59c3"               # every other key is a token the theme declared
//!
//! [site]
//! home = "README.md"               # pins site.home; false pins a generated listing
//!
//! [page."posts/old/x.md"]
//! title = "On shelves"             # pins page.title on posts/old/x.md
//! ```
//!
//! [`Config::parse`] never fails: problems are diagnostics with spans, and a key with a problem
//! is not applied. [`Config::pin`] gives the pin of one code, typed. [`pins`] and [`pins_into`]
//! go the other way, from decisions to the file. Every span is a byte range in the text given
//! to [`Config::parse`]; the caller knows which file that was and attaches the subject.
//!
//! The `toml` crate is seen only by `read`, `toml_edit` only by `write`.

mod pattern;
mod pin;
mod read;
mod theme;
mod value;
mod write;

use std::collections::BTreeMap;
use std::ops::Range;

use model::{AnyCode, AnyDecision, AnyOf, Code, ContentPath, Decided, Diagnostic, Each, Evidence, Span, Subject};

pub use pattern::{Pattern, PatternError, Patterns};
pub use pin::Pinned;
pub use theme::{IdPart, Theme, ThemeId, ThemeIdError, Token};
pub use write::{Pinnable, PinsError, SyntaxError, Unwritable, pins, pins_into};

/// What `ink.toml` says. The default is what no `ink.toml` says: no pins, the default theme,
/// no patterns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// Every pin, by subject (the site first, then files in path order) and code.
    pins: BTreeMap<Subject, BTreeMap<AnyCode, AnyOf<pin::Pins>>>,
    theme: Option<Theme>,
    include: Patterns,
    exclude: Patterns,
}

/// The result of reading `ink.toml`: what applied, and every problem found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Every key that read cleanly.
    pub config: Config,
    /// Every problem, in the order the keys were written.
    pub diagnostics: Vec<Diagnostic>,
}

impl Config {
    /// Reads `ink.toml`. A syntax error gives the default config and one `config.syntax.invalid`
    /// per error; an unknown key is `config.key.unknown` and a value of the wrong shape is
    /// `config.value.invalid`, and neither is applied.
    pub fn parse(text: &str) -> Parsed {
        read::Reader::parse(text)
    }

    /// The pin of code `C` on `subject`, if `ink.toml` has one.
    pub fn pin<C: Code>(&self, subject: &C::Subject) -> Option<&Pinned<C>> {
        let subject: Subject = subject.clone().into();
        self.pins.get(&subject)?.get(&C::CODE)?.get::<C>()
    }

    /// The pin of `code` on `subject` as the untyped value the engine's `Pins` takes today, or
    /// `None` when there is none or `subject` is of the other scope.
    // HACK(the engine's `Pins` and the CLI's `Configured` move onto `Config::pin::<C>`, phase 3):
    // the one bridge from typed pins to `Decided`; remove it with them.
    pub fn decided(&self, code: AnyCode, subject: &Subject) -> Option<(Decided, Span)> {
        code.each(&mut Untyped { config: self, subject })
    }

    /// Every file `ink.toml` pins something on, in path order, so a caller can say when one of
    /// them is not in the snapshot.
    pub fn pinned_files(&self) -> impl Iterator<Item = &ContentPath> {
        self.pins.keys().filter_map(|subject| match subject {
            Subject::File(path) => Some(path),
            _ => None,
        })
    }

    /// The `[theme]` table, if there is one.
    pub fn theme(&self) -> Option<&Theme> {
        self.theme.as_ref()
    }

    /// `include = [...]`: files to publish that inference would leave out.
    pub fn include(&self) -> &Patterns {
        &self.include
    }

    /// `exclude = [...]`: files that are not part of the site.
    pub fn exclude(&self) -> &Patterns {
        &self.exclude
    }

    /// Records `pinned` for `C` on `subject`.
    pub(crate) fn insert<C: Code>(&mut self, subject: Subject, pinned: Pinned<C>) {
        self.pins.entry(subject).or_default().insert(C::CODE, AnyOf::new::<C>(pinned));
    }
}

/// [`Config::decided`] for one code.
struct Untyped<'a> {
    config: &'a Config,
    subject: &'a Subject,
}

impl<C: Code> Each<C> for Untyped<'_> {
    type Out = Option<(Decided, Span)>;

    fn each(&mut self) -> Self::Out {
        let subject = C::Subject::try_from(self.subject.clone()).ok()?;
        let pinned = self.config.pin::<C>(&subject)?;
        let span = pinned.span;
        let typed = model::decision::Decision::<C>::new(subject, pinned.value.clone(), Evidence::Config { span });
        let untyped = model::Decision::from(AnyDecision::from(typed));
        Some((untyped.decided, span))
    }
}

/// A byte range toml or `toml_edit` gives, as a [`Span`]. They never give one that ends before
/// it starts; one that did would become the empty span at its start.
#[derive(Clone, Debug)]
pub(crate) struct Offsets(pub(crate) Range<usize>);

/// The empty span at the start of the text.
const START: Span = match Span::new(0, 0) {
    Ok(span) => span,
    // evaluated at compile time, and 0 is not after 0
    Err(_) => panic!("an empty span"),
};

impl From<Offsets> for Span {
    fn from(Offsets(range): Offsets) -> Self {
        Self::new(range.start, range.end.max(range.start)).unwrap_or(START)
    }
}
