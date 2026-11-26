//! The typed fields of an entry, and the `[field]` section that documents each field name once:
//! a name means the same thing in every entry that has it, so its doc is written once and every
//! generated field carries it.

use std::collections::BTreeMap;

use super::{Flaw, Named};

/// One typed field of an entry.
#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) kind: Kind,
    /// What it holds, from `[field]`.
    pub(crate) doc: String,
}

/// The closed set of field types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Path,
    Paths,
    Route,
    Text,
    Count,
    Bytes,
    Span,
    Decision,
}

impl Named for Kind {
    const NAMES: &'static [(&'static str, Self)] = &[
        ("path", Self::Path),
        ("paths", Self::Paths),
        ("route", Self::Route),
        ("text", Self::Text),
        ("count", Self::Count),
        ("bytes", Self::Bytes),
        ("span", Self::Span),
        ("decision", Self::Decision),
    ];
}

/// The `[field]` section: each field name's doc, and which names an entry has used.
#[derive(Debug, Default)]
pub(crate) struct Docs {
    docs: BTreeMap<String, String>,
    used: Vec<String>,
}

impl Docs {
    /// Reads `[field]`, a table of names to text, each flaw into `flaws`.
    pub(crate) fn read(section: toml::Table, flaws: &mut Vec<Flaw>) -> Self {
        let mut docs = BTreeMap::new();
        for (name, doc) in section {
            match doc {
                toml::Value::String(doc) => {
                    docs.insert(name, doc);
                }
                _ => flaws.push(Flaw::FieldDoc { field: name }),
            }
        }
        Self { docs, used: Vec::new() }
    }

    /// The doc of the field `name` of `code`, or a flaw saying it has none.
    pub(crate) fn of(&mut self, code: &str, name: &str, flaws: &mut Vec<Flaw>) -> Option<String> {
        let found = self.docs.get(name).cloned();
        match &found {
            Some(_) if !self.used.iter().any(|used| used == name) => self.used.push(name.to_owned()),
            Some(_) => {}
            None => flaws.push(Flaw::Undocumented { code: code.to_owned(), field: name.to_owned() }),
        }
        found
    }

    /// A flaw for every documented name no entry has.
    pub(crate) fn unused(&self, flaws: &mut Vec<Flaw>) {
        let unused = self.docs.keys().filter(|name| !self.used.contains(name));
        flaws.extend(unused.map(|name| Flaw::Unused { field: name.clone() }));
    }
}
