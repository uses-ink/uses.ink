//! The catalog as checked data: entries, the keys and sections they are written under, and the sets
//! of words they choose from.

use std::fmt;

use super::{Field, Flaw, Template};

/// A checked catalog: diagnostics and notices, in file order.
#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) diagnostics: Vec<Entry<Rank>>,
    pub(crate) notices: Vec<Entry<()>>,
}

/// One code of the catalog. `C` is what its class adds: a diagnostic's [`Rank`], nothing for a
/// notice.
#[derive(Debug)]
pub(crate) struct Entry<C> {
    pub(crate) code: String,
    /// The Rust variant: `link.target.ambiguous` is `LinkTargetAmbiguous`.
    pub(crate) variant: String,
    pub(crate) class: C,
    pub(crate) fields: Vec<Field>,
    pub(crate) message: Template,
    pub(crate) fix: Option<Template>,
}

/// What a diagnostic has that a notice does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rank {
    pub(crate) severity: Severity,
    pub(crate) fault: Fault,
}

/// A closed set the catalog spells as text: each member and its name.
pub(crate) trait Named: Copy + 'static {
    /// Every member, by name.
    const NAMES: &'static [(&'static str, Self)];

    /// The member named `name`.
    fn find(name: &str) -> Option<Self> {
        Self::NAMES.iter().find(|(n, _)| *n == name).map(|&(_, member)| member)
    }
}

/// A [`Named`] set an entry chooses from under one key, refused with a flaw of its own.
pub(crate) trait Choice: Named {
    /// The flaw for `given`, which names no member.
    fn refuse(code: String, given: String) -> Flaw;
}

/// The sections of the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Diagnostic,
    Notice,
}

impl Named for Section {
    const NAMES: &'static [(&'static str, Self)] = &[("diagnostic", Self::Diagnostic), ("notice", Self::Notice)];
}

impl Section {
    /// `[field]`, read before the others: each field name's doc.
    pub(super) const FIELD: &'static str = "field";
}

/// The keys an entry may have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Severity,
    Fault,
    Fields,
    Message,
    Fix,
}

impl Key {
    const ALL: [Self; 5] = [Self::Severity, Self::Fault, Self::Fields, Self::Message, Self::Fix];

    /// The key named `name`.
    pub(super) fn find(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }

    /// Its name in the catalog.
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Severity => "severity",
            Self::Fault => "fault",
            Self::Fields => "fields",
            Self::Message => "message",
            Self::Fix => "fix",
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
    Note,
}

impl Named for Severity {
    const NAMES: &'static [(&'static str, Self)] =
        &[("error", Self::Error), ("warning", Self::Warning), ("note", Self::Note)];
}

impl Choice for Severity {
    fn refuse(code: String, given: String) -> Flaw {
        Flaw::Severity { code, given }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    Author,
    Source,
    Operator,
    Ours,
}

impl Named for Fault {
    const NAMES: &'static [(&'static str, Self)] =
        &[("author", Self::Author), ("source", Self::Source), ("operator", Self::Operator), ("ours", Self::Ours)];
}

impl Choice for Fault {
    fn refuse(code: String, given: String) -> Flaw {
        Flaw::Fault { code, given }
    }
}
