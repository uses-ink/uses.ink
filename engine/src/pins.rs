//! What the engine asks of `ink.toml`. The engine mirrors the config crate's API in a trait so it
//! builds and tests on its own; `uses-ink-config` implements it.

use model::{ContentPath, Decided, DecisionCode, Span, Subject};

/// The author's pins and their include and exclude patterns.
pub trait Pins {
    /// The value pinned for `code` on `subject`, if `ink.toml` pins one.
    fn pin(&self, code: DecisionCode, subject: &Subject) -> Option<Pin>;
    /// Where the first `include` pattern matching `path` is written, if one does.
    fn included(&self, path: &ContentPath) -> Option<Span>;
    /// Where the first `exclude` pattern matching `path` is written, if one does.
    fn excluded(&self, path: &ContentPath) -> Option<Span>;
}

/// A pinned value and where in `ink.toml` it is written. It becomes the decision, with
/// `Evidence::Config { span }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub decided: Decided,
    pub span: Span,
}

/// A site with no `ink.toml`: nothing is pinned, nothing included or excluded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoPins;

impl Pins for NoPins {
    fn pin(&self, _code: DecisionCode, _subject: &Subject) -> Option<Pin> {
        None
    }

    fn included(&self, _path: &ContentPath) -> Option<Span> {
        None
    }

    fn excluded(&self, _path: &ContentPath) -> Option<Span> {
        None
    }
}
