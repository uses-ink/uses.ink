//! The pipeline, as `ink build` runs it: read, interpret, render.

use std::collections::BTreeMap;
use std::path::Path;

use build::{Event, Events};
use config::{Config, Parsed};
use engine::{BaseUrl, Blobs, Chosen, Engine, NoPins, RenderSettings};
use host_image::Pixels;
use host_native::{Folder, Grammars, Memory, Read, Repository, Revision};
use model::record::PluginId;
use model::{Bytes, ContentPath, Diagnostic, Hash, Manifest, Notice, Problem, Severity, Snapshot, Subject};
use theme_quiet::Quiet;

use crate::pins::Pinned;
use crate::reader::Reader;

/// The config file's path in the snapshot.
const CONFIG: &str = "ink.toml";

/// What a build made, before its blobs are stored.
#[derive(Debug)]
pub(crate) struct Made {
    pub(crate) manifest: Manifest,
    /// Blobs the render made; the others are the snapshot's, in `store`.
    pub(crate) blobs: BTreeMap<Hash, Vec<u8>>,
    pub(crate) store: Memory,
}

/// A folder to build, and where its site will be served.
#[derive(Debug)]
pub(crate) struct Pipeline<'a> {
    pub(crate) dir: &'a Path,
    pub(crate) base: BaseUrl,
}

impl Pipeline<'_> {
    /// Reads the folder: as a repository when it holds `.git`, so dates come from history, else
    /// as plain files. A refusal is the diagnostic that says why.
    fn read(&self, store: &mut Memory) -> Result<Read, Diagnostic> {
        let versioned = std::fs::symlink_metadata(self.dir.join(".git")).is_ok();
        let read = if versioned {
            Repository { revision: Revision::Head, ..Repository::new(self.dir) }.read(store)
        } else {
            Folder::new(self.dir).read(store)
        };
        read.map_err(|refused| Diagnostic::from(&refused))
    }

    /// `ink.toml` of the snapshot, parsed; its problems are about the file.
    fn parse(snapshot: &Snapshot, store: &Memory) -> Parsed {
        let mut parsed = Parsed::default();
        let path: Option<ContentPath> = CONFIG.parse().ok();
        let entry = path.as_ref().and_then(|path| snapshot.entry(path).map(|entry| (path, entry.blob)));
        let Some((path, blob)) = entry else { return parsed };
        let subject = Subject::File(path.clone());
        match store.read(&blob).map(|bytes| String::from_utf8(bytes.into_owned())) {
            Ok(Ok(text)) => {
                parsed = Config::parse(&text);
                for diagnostic in &mut parsed.diagnostics {
                    diagnostic.subject = Some(subject.clone());
                }
            }
            Ok(Err(error)) => {
                let problem = Problem::SourceFileEncoding { at: error.utf8_error().valid_up_to() as u64 };
                parsed.diagnostics.push(Diagnostic::new(problem).on(subject));
            }
            Err(_) => {
                let problem = Problem::SourceBlobMissing { path: path.clone() };
                parsed.diagnostics.push(Diagnostic::new(problem).on(subject));
            }
        }
        parsed
    }

    /// Whether these diagnostics stop the build.
    fn stops(diagnostics: &[Diagnostic]) -> bool {
        diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error)
    }

    /// Says what the engine said, as events.
    fn say(events: &mut impl Events, diagnostics: &[Diagnostic], notices: &[Notice]) {
        diagnostics.iter().for_each(|diagnostic| events.event(Event::Diagnosed(diagnostic.clone())));
        notices.iter().for_each(|notice| events.event(Event::Noticed(notice.clone())));
    }

    /// Runs the pipeline, saying everything to `events`. `Ok(None)` is a site that does not
    /// build; its diagnostics have been said. An error is a format pinned that is not held.
    pub(crate) fn run(&self, events: &mut impl Events) -> Result<Option<Made>, PluginId> {
        let mut store = Memory::default();
        let read = match self.read(&mut store) {
            Ok(read) => read,
            Err(diagnostic) => {
                events.event(Event::Refused(diagnostic));
                return Ok(None);
            }
        };
        let snapshot = read.snapshot;
        events.event(Event::SourceFetched {
            version: snapshot.version().clone(),
            files: snapshot.tree().len() as u64,
            bytes: Bytes(snapshot.tree().iter().map(|entry| entry.size.count()).sum()),
            observed: snapshot.observed().clone(),
        });
        let Parsed { config, diagnostics } = Self::parse(&snapshot, &store);
        let pins = Pinned(&config);
        let chosen = Chosen::new(&snapshot, &pins, None);
        let reader = Reader::try_from(chosen.format())?;
        let mut understood = Engine::new(&store, pins, (reader,), ()).interpret(&snapshot).with(chosen);
        let stopped = Self::stops(&diagnostics);
        understood.diagnostics.splice(0..0, diagnostics);
        understood.notices.splice(0..0, read.skipped.iter().map(Notice::from));
        understood.decisions.iter().for_each(|decision| events.event(Event::Decided(decision.clone())));
        Self::say(events, &understood.diagnostics, &understood.notices);
        let Some(site) = understood.site.filter(|_| !stopped) else { return Ok(None) };
        let theme = Quiet::new(self.base.clone(), Grammars::default());
        let rendered = Engine::new(&store, NoPins, (), &theme).render(&site, &RenderSettings { base: self.base.clone() }, &Pixels::default());
        Self::say(events, &rendered.diagnostics, &rendered.notices);
        if Self::stops(&rendered.diagnostics) {
            return Ok(None);
        }
        Ok(Some(Made { manifest: rendered.manifest, blobs: rendered.blobs, store }))
    }
}
