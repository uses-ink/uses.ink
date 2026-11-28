//! Settings, paths and the answers read back from what was understood.

use engine::{RenderSettings, Understood};
use model::code::Code;
use model::decision::{self, Decision};
use model::{ContentPath, Diagnostic, Subject};

pub fn settings() -> RenderSettings {
    RenderSettings { base: "https://ink.uses.ink".parse().unwrap() }
}

pub fn path(text: &str) -> ContentPath {
    text.parse().unwrap()
}

pub fn subject(text: &str) -> Subject {
    Subject::File(path(text))
}

/// The codes of every diagnostic.
pub fn codes(understood: &Understood) -> Vec<&'static str> {
    understood.diagnostics.iter().map(Diagnostic::code).collect()
}

/// The decision of `C` on `subject`.
fn decided<C: Code>(understood: &Understood, subject: &C::Subject) -> Decision<C> {
    understood.decisions.iter().find_map(|d| d.get::<C>().filter(|d| &d.subject == subject)).cloned().unwrap()
}

/// The decision of `C` on the file at `path`.
pub fn on<C: Code<Subject = decision::File>>(understood: &Understood, path: &str) -> Decision<C> {
    decided::<C>(understood, &decision::File(self::path(path)))
}

/// The decision of `C` on the site.
pub fn site<C: Code<Subject = decision::Site>>(understood: &Understood) -> Decision<C> {
    decided::<C>(understood, &decision::Site)
}
