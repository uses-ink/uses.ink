//! Pins, includes and excludes held in lists.

use engine::{Patterns, Pin, Pins};
use model::code::{AnyDecision, Code};
use model::decision::{self, Decision};
use model::{ContentPath, Evidence, PageKind, Section, Span};

/// Pins, includes and excludes, held in lists, the pins erased as storage holds them. A pattern
/// matches a path it prefixes.
#[derive(Debug, Default)]
pub struct Pinned {
    pins: Vec<AnyDecision>,
    include: Vec<String>,
    exclude: Vec<String>,
    folders: Vec<(String, PageKind)>,
    sections: Vec<Section>,
}

fn span() -> Span {
    Span::new(0, 1).unwrap()
}

impl Pinned {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn pin<C: Code>(mut self, subject: C::Subject, value: C::Value) -> Self {
        let decision = Decision::<C>::new(subject, value, Evidence::Config { span: span() });
        self.pins.push(decision.into());
        self
    }

    pub fn site<C: Code<Subject = decision::Site>>(self, value: C::Value) -> Self {
        self.pin::<C>(decision::Site, value)
    }

    pub fn file<C: Code<Subject = decision::File>>(self, path: &str, value: C::Value) -> Self {
        self.pin::<C>(decision::File(path.parse().unwrap()), value)
    }

    pub fn include(mut self, prefix: &str) -> Self {
        self.include.push(prefix.to_owned());
        self
    }

    /// A `[folder]` table that gives the pages under `folder` the kind `kind`.
    pub fn folder(mut self, folder: &str, kind: PageKind) -> Self {
        self.folders.push((folder.to_owned(), kind));
        self
    }

    /// A `[folder]` table that says a weight or an order.
    pub fn section(mut self, section: Section) -> Self {
        self.sections.push(section);
        self
    }

    pub fn exclude(mut self, prefix: &str) -> Self {
        self.exclude.push(prefix.to_owned());
        self
    }
}

impl Pins for Pinned {
    fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>> {
        let found = self.pins.iter().find_map(|d| d.get::<C>().filter(|d| &d.subject == subject))?;
        Some(Pin { value: found.value.clone(), span: span() })
    }
}

impl Patterns for Pinned {
    fn included(&self, path: &ContentPath) -> Option<Span> {
        self.include.iter().any(|p| path.as_str().starts_with(p.as_str())).then(span)
    }

    fn excluded(&self, path: &ContentPath) -> Option<Span> {
        self.exclude.iter().any(|p| path.as_str().starts_with(p.as_str())).then(span)
    }

    fn folder_kind(&self, path: &ContentPath) -> Option<(PageKind, Span)> {
        let held = |folder: &&(String, PageKind)| path.as_str().starts_with(&format!("{}/", folder.0));
        self.folders.iter().filter(held).max_by_key(|folder| folder.0.len()).map(|folder| (folder.1, span()))
    }

    fn sections(&self) -> Vec<Section> {
        self.sections.clone()
    }
}
