//! A snapshot written file by file.

use engine::{Engine, Format, NoPins, Pins, Rendered, Theme, Understood};
use model::{Ability, Bytes, Entry, FileFacts, Hash, Known, Observed, Snapshot, UnknownReason, Version};
use std::collections::BTreeMap;

use super::lines::Lines;
use super::{Pinned, Stand, Store, settings};

/// A snapshot written file by file.
#[derive(Debug, Default)]
pub struct Tree {
    files: BTreeMap<String, Vec<u8>>,
    facts: Vec<FileFacts>,
    observed: Observed,
    missing: Vec<String>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_owned(), text.as_bytes().to_vec());
        self
    }

    /// A file whose bytes the store does not hold.
    pub fn lost(mut self, path: &str) -> Self {
        self.missing.push(path.to_owned());
        self.file(path, "lost")
    }

    pub fn created(mut self, path: &str, created: Known<model::Timestamp>) -> Self {
        let unknown = Known::Unknown { reason: UnknownReason::NoHistory };
        self.facts.push(FileFacts::new(path.parse().unwrap(), created, unknown));
        self
    }

    /// A source with history, lacking it before a shallow clone's boundary.
    pub fn shallow(mut self) -> Self {
        let bound = "2023-01-01T00:00:00Z".parse().unwrap();
        let gap = model::Gap::Shallow { commit: model::Oid::try_from("a".repeat(40)).unwrap(), bound };
        let lack = model::Lack::new(Ability::History, model::AnyCode::Date, gap);
        self.observed = Observed::new(vec![Ability::Snapshot, Ability::History], vec![lack]);
        self
    }

    /// A source with its whole history.
    pub fn deep(mut self) -> Self {
        self.observed = Observed::new(vec![Ability::Snapshot, Ability::History], vec![]);
        self
    }

    pub fn private(mut self) -> Self {
        self.observed = Observed::new(vec![Ability::Snapshot, Ability::Private], vec![]);
        self
    }

    pub fn snapshot(&self) -> (Snapshot, Store) {
        let mut store = Store::default();
        let tree = self
            .files
            .iter()
            .map(|(path, bytes)| {
                let hash = Hash::of(bytes);
                if !self.missing.contains(path) {
                    store.0.insert(hash, bytes.clone());
                }
                Entry::new(path.parse().unwrap(), hash, Bytes(bytes.len() as u64))
            })
            .collect();
        let version = Version::Folder { hash: Hash::of(b"tree") };
        (Snapshot::new(version, tree, self.facts.clone(), self.observed.clone()).unwrap(), store)
    }

    pub fn understand(&self, pins: &Pinned) -> Understood {
        self.read_with(pins, Lines)
    }

    /// The record of a build of the tree under `pins`, by this engine with no plugin.
    pub fn record(&self, pins: &Pinned) -> model::record::Build {
        let (snapshot, _) = self.snapshot();
        let inputs = model::record::Inputs::of(&snapshot, engine::RELEASE.into(), Vec::new(), None);
        self.understand(pins).record(&snapshot, inputs)
    }

    /// Interprets the tree, tracing the ladder of `code` on `subject`.
    pub fn trace(&self, pins: &Pinned, code: model::AnyCode, subject: model::Subject) -> Understood {
        let (snapshot, store) = self.snapshot();
        Engine::new(&store, pins, (Lines,), ()).trace(&snapshot, code, subject)
    }

    /// Interprets the tree through `format`.
    pub fn read_with(&self, pins: &impl Pins, format: impl Format) -> Understood {
        let (snapshot, store) = self.snapshot();
        Engine::new(&store, pins, (format,), ()).interpret(&snapshot)
    }
}

impl Tree {
    /// Interprets the tree and renders it through `theme`, its images through [`Stand`].
    pub fn render(&self, theme: &impl Theme) -> Rendered {
        let (_, store) = self.snapshot();
        let site = self.understand(&Pinned::none()).site.unwrap();
        Engine::new(&store, NoPins, (), theme).render(&site, &settings(), &Stand)
    }
}
