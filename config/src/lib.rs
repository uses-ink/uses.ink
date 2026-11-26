//! `ink.toml`, the one optional file at a source's root: pins of decisions, the theme choice and
//! which files are part of the site.
//!
//! **The decision code is the `ink.toml` path.** A sitewide code `site.home` is pinned by
//! `[site] home = ...`; a per-file code `page.title` on `posts/x.md` by
//! `[page."posts/x.md"] title = ...`, and `file.ignore` by `[file."x"] ignore = true`. The
//! table and key come from [`DecisionCode::name`], so nothing here lists a pin by hand.
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
//! is not applied. [`pins`] and [`pins_into`] go the other way, from decisions to the file.
//! Every span is a byte range in the text given to [`Config::parse`]; the caller knows which
//! file that was and attaches the subject.
//!
//! [`DecisionCode::name`]: model::DecisionCode::name

mod pattern;
mod place;
mod read;
mod shape;
mod theme;
mod value;
mod write;

use std::collections::BTreeMap;

use model::{ContentPath, Decided, DecisionCode, Diagnostic, Span, Spanned, Subject};

pub use pattern::{Pattern, PatternError};
pub use theme::{IdPart, Theme, ThemeId, ThemeIdError, Token};
pub use write::{PinsError, SyntaxError, Unwritable, pins, pins_into};

/// What `ink.toml` says. The default is what no `ink.toml` says: no pins, the default theme,
/// no patterns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    site: BTreeMap<DecisionCode, Pin>,
    files: BTreeMap<ContentPath, BTreeMap<DecisionCode, Pin>>,
    theme: Option<Theme>,
    include: Vec<Spanned<Pattern>>,
    exclude: Vec<Spanned<Pattern>>,
}

/// The result of reading `ink.toml`: what applied, and every problem found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Every key that read cleanly.
    pub config: Config,
    /// Every problem, in the order the keys were written.
    pub diagnostics: Vec<Diagnostic>,
}

/// A pinned value and where it was written (`key = value`): a decision's `Evidence::Config`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    /// The pinned value; its variant is the code.
    pub decided: Decided,
    /// The whole `key = value`.
    pub span: Span,
}

impl Config {
    /// Reads `ink.toml`. A syntax error gives the default config and one `config.syntax.invalid`
    /// per error; an unknown key is `config.key.unknown` and a value of the wrong shape is
    /// `config.value.invalid`, and neither is applied.
    pub fn parse(text: &str) -> Parsed {
        read::parse(text)
    }

    /// The pin of `code` on `subject`, if `ink.toml` has one. A sitewide code is pinned only on
    /// the site, a per-file code only on a file.
    pub fn pin(&self, code: DecisionCode, subject: &Subject) -> Option<&Pin> {
        match subject {
            Subject::Site if code.is_sitewide() => self.site.get(&code),
            Subject::File(path) if !code.is_sitewide() => self.files.get(path)?.get(&code),
            _ => None,
        }
    }

    /// Every file `ink.toml` pins something on, in path order, so a caller can say when one of
    /// them is not in the snapshot.
    pub fn pinned_files(&self) -> impl Iterator<Item = &ContentPath> {
        self.files.keys()
    }

    /// The `[theme]` table, if there is one.
    pub fn theme(&self) -> Option<&Theme> {
        self.theme.as_ref()
    }

    /// `include = [...]`: files to publish that inference would leave out.
    pub fn include(&self) -> &[Spanned<Pattern>] {
        &self.include
    }

    /// `exclude = [...]`: files that are not part of the site.
    pub fn exclude(&self) -> &[Spanned<Pattern>] {
        &self.exclude
    }
}
