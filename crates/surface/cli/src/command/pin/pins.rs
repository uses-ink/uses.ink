//! `ink.toml` as the engine's [`Pins`]. The engine mirrors config's API in a trait so it builds
//! on its own; the two meet here.

use config::Config;
use engine::{Patterns, Pin, Pins};
use model::{Code, ContentPath, PageKind, Section, Span};

/// The pins, includes and excludes of one `ink.toml`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Configured<'a>(pub(crate) &'a Config);

impl Pins for Configured<'_> {
    fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>> {
        self.0.pin::<C>(subject).map(|pinned| Pin { value: pinned.value.clone(), span: pinned.span })
    }
}

impl Patterns for Configured<'_> {
    fn included(&self, path: &ContentPath) -> Option<Span> {
        self.0.include().find(path).map(|pattern| pattern.span)
    }

    fn excluded(&self, path: &ContentPath) -> Option<Span> {
        self.0.exclude().find(path).map(|pattern| pattern.span)
    }

    fn folder_kind(&self, path: &ContentPath) -> Option<(PageKind, Span)> {
        self.0.folder_kind(path)
    }

    fn sections(&self) -> Vec<Section> {
        self.0.sections()
    }
}
