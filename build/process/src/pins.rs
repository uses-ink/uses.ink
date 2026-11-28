//! `ink.toml` as the engine's pins.

use config::Config;
use engine::{Patterns, Pin, Pins};
use model::{Code, ContentPath, Span};

/// The pins, includes and excludes of one `ink.toml`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pinned<'a>(pub(crate) &'a Config);

impl Pins for Pinned<'_> {
    fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>> {
        self.0.pin::<C>(subject).map(|pinned| Pin { value: pinned.value.clone(), span: pinned.span })
    }
}

impl Patterns for Pinned<'_> {
    fn included(&self, path: &ContentPath) -> Option<Span> {
        self.0.include().find(path).map(|pattern| pattern.span)
    }

    fn excluded(&self, path: &ContentPath) -> Option<Span> {
        self.0.exclude().find(path).map(|pattern| pattern.span)
    }
}
