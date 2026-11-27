//! The pipeline, as `ink build` runs it: read, interpret, render.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;

use build::{Event, Events};
use config::{Config, Parsed};
use engine::{BaseUrl, Blobs, Chosen, Engine, NoPins, RenderSettings};
use host_image::Pixels;
use host_native::{Folder, Grammars, Memory, Read, Repository, Revision};
use model::record::PluginId;
use model::{Bytes, ContentPath, Diagnostic, Hash, Manifest, Notice, Problem, Severity, Snapshot, Subject};
use theme_quiet::Quiet;
use trace::Field as _;

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
pub(crate) struct Pipeline {
    pub(crate) base: BaseUrl,
}

impl Pipeline {
    /// Reads the folder: as a repository when it holds `.git`, so dates come from history, else
    /// as plain files. A refusal is the diagnostic that says why.
    pub(crate) fn read(dir: &Path, store: &mut Memory) -> Result<Read, Box<Diagnostic>> {
        let versioned = match std::fs::symlink_metadata(dir.join(".git")) {
            Ok(_) => true,
            Err(source) if matches!(source.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => false,
            Err(source) => {
                let problem =
                    Problem::SourceRepositoryUnreadable { reason: format!("`.git` could not be inspected: {source}") };
                return Err(Box::new(Diagnostic::new(problem)));
            }
        };
        trace::debug!(
            name: trace::event::SOURCE_OPENED,
            kind = if versioned { "git" } else { "folder" },
            path = trace::Located(dir).field(),
        );
        let read = if versioned {
            Repository { revision: Revision::Head, ..Repository::new(dir) }.read(store)
        } else {
            Folder::new(dir).read(store)
        };
        read.map_err(|refused| Box::new(Diagnostic::from(&refused)))
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
        for diagnostic in diagnostics {
            events.event(Event::Diagnosed(diagnostic.clone()));
        }
        for notice in notices {
            events.event(Event::Noticed(notice.clone()));
        }
    }

    /// Says the snapshot arrived: to the log, and as an event.
    fn arrived(events: &mut impl Events, snapshot: &Snapshot) {
        let bytes = Bytes(snapshot.tree().iter().map(|entry| entry.size.count()).sum());
        trace::debug!(
            name: trace::event::SOURCE_FETCHED,
            files = trace::Count::from(snapshot.tree().len()).field(),
            bytes = trace::Size(bytes.count()).field(),
        );
        events.event(Event::SourceFetched {
            version: snapshot.version().clone(),
            files: snapshot.tree().len() as u64,
            bytes,
            observed: snapshot.observed().clone(),
        });
    }

    /// Runs the pipeline on what was `read` into `store`, saying everything to `events`.
    /// `Ok(None)` is a site that does not build; its diagnostics have been said. An error is a
    /// format pinned that is not held.
    pub(crate) fn run(
        &self,
        read: Result<Read, Box<Diagnostic>>,
        store: Memory,
        events: &mut impl Events,
    ) -> Result<Option<Made>, PluginId> {
        let read = match read {
            Ok(read) => read,
            Err(diagnostic) => {
                trace::info!(name: trace::event::BUILD_END, outcome = "refused-at-source");
                events.event(Event::Refused(*diagnostic));
                return Ok(None);
            }
        };
        let snapshot = read.snapshot;
        Self::arrived(events, &snapshot);
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
        let rendered = Engine::new(&store, NoPins, (), &theme).render(
            &site,
            &RenderSettings { base: self.base.clone() },
            &Pixels::default(),
        );
        Self::say(events, &rendered.diagnostics, &rendered.notices);
        if Self::stops(&rendered.diagnostics) {
            return Ok(None);
        }
        Ok(Some(Made { manifest: rendered.manifest, blobs: rendered.blobs, store }))
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn an_unreadable_git_entry_is_refused_not_read_as_plain_files() {
        let folder = tempfile::tempdir().unwrap();
        let dir = folder.path().join("site");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        let readable = std::fs::read_dir(&dir).is_ok();
        let refused = Pipeline::read(&dir, &mut Memory::default()).err();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        if readable {
            // running as a user the mode does not bind: nothing to observe
            return;
        }
        let refused = refused.unwrap();
        assert!(matches!(refused.problem, Problem::SourceRepositoryUnreadable { .. }), "{refused:?}");
    }
}
