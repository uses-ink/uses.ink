//! `ink.toml` as the engine's [`Pins`]. The engine mirrors config's API in a trait so it builds
//! on its own; the two meet here.

use config::Config;
use engine::{Pin, Pins};
use model::{ContentPath, DecisionCode, Span, Subject};

/// The pins, includes and excludes of one `ink.toml`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Configured<'a>(pub(crate) &'a Config);

impl Pins for Configured<'_> {
    fn pin(&self, code: DecisionCode, subject: &Subject) -> Option<Pin> {
        self.0.pin(code, subject).map(|pin| Pin { decided: pin.decided.clone(), span: pin.span })
    }

    fn included(&self, path: &ContentPath) -> Option<Span> {
        self.0.include().iter().find(|pattern| pattern.value.matches(path)).map(|pattern| pattern.span)
    }

    fn excluded(&self, path: &ContentPath) -> Option<Span> {
        self.0.exclude().iter().find(|pattern| pattern.value.matches(path)).map(|pattern| pattern.span)
    }
}
