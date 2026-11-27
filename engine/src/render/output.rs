//! What a render has produced so far: one entry per route with the policy's headers, the bytes
//! behind new blobs, and what was said along the way.

use std::collections::BTreeMap;
use std::io;
use trace::Field as _;

use model::{ContentPath, Diagnostic, Hash, Manifest, MediaType, Notice, Route, Served, Status};

use crate::Record;
use crate::error::Error;
use crate::render::policy::{Cache, Policy};
use crate::render::settings::BaseUrl;
use crate::render::xml::Xml;
use crate::render::{Outcome, Rendered, Reserved, Step, Unit};

/// Who serves a route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    /// A page, from its file.
    Page(ContentPath),
    /// A plain file of the snapshot.
    Asset(ContentPath),
    /// The engine: feeds, the sitemap, `robots.txt`, the not-found page.
    Engine,
    /// The theme's own files.
    Theme,
}

/// What an entry serves.
#[derive(Debug)]
pub(crate) enum Body {
    /// New bytes, stored under their hash.
    Bytes(Vec<u8>),
    /// A blob the snapshot already stored.
    Blob(Hash),
}

/// One entry to serve.
#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) route: Route,
    pub(crate) media: MediaType,
    pub(crate) status: Status,
    pub(crate) body: Body,
    pub(crate) owner: Owner,
    pub(crate) cache: Cache,
}

impl Record for Output<'_> {
    fn diagnose(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }
}

/// The render so far.
#[derive(Debug)]
pub(crate) struct Output<'a> {
    policy: &'a Policy,
    /// Where the site is served, for absolute links.
    pub(crate) base: &'a BaseUrl,
    entries: BTreeMap<Route, (Served, Owner)>,
    blobs: BTreeMap<Hash, Vec<u8>>,
    diagnostics: Vec<Diagnostic>,
    notices: Vec<Notice>,
    steps: Vec<Step>,
}

impl<'a> Output<'a> {
    /// Nothing served yet, headers from `policy`, absolute links under `base`.
    pub(crate) fn new(policy: &'a Policy, base: &'a BaseUrl) -> Self {
        let (entries, blobs) = (BTreeMap::new(), BTreeMap::new());
        Self { policy, base, entries, blobs, diagnostics: Vec::new(), notices: Vec::new(), steps: Vec::new() }
    }

    /// How much has been said so far, diagnostics and notices together: a page that left it as
    /// it was had nothing to say.
    pub(crate) fn said(&self) -> usize {
        self.diagnostics.len() + self.notices.len()
    }

    /// Records what became of one unit.
    pub(crate) fn step(&mut self, unit: Unit, outcome: Outcome) {
        self.steps.push(Step::new(unit, outcome));
    }

    /// How many units were taken from the cache and how many were rendered.
    pub(crate) fn tally(&self) -> (u64, u64) {
        let reused = self.steps.iter().filter(|step| step.outcome == Outcome::Reused).count() as u64;
        (reused, self.steps.len() as u64 - reused)
    }

    /// Who already serves `route`.
    pub(crate) fn owner(&self, route: &Route) -> Option<&Owner> {
        self.entries.get(route).map(|(_, owner)| owner)
    }

    /// Serves `entry` with the policy's headers for its media type. A route served twice is a
    /// bug: the first entry stays.
    pub(crate) fn serve(&mut self, entry: Entry) {
        if self.entries.contains_key(&entry.route) {
            self.fail(Error::Served { route: entry.route });
            return;
        }
        let blob = match entry.body {
            Body::Bytes(bytes) => {
                let hash = Hash::of(&bytes);
                self.blobs.insert(hash, bytes);
                hash
            }
            Body::Blob(hash) => hash,
        };
        let served = Served::new(entry.route.clone(), blob, entry.media, self.policy.headers(entry.media, entry.cache));
        self.entries.insert(entry.route, (served.with_status(entry.status), entry.owner));
    }

    /// Serves the XML `write` writes at the engine's route `at`. `write` is given the route's
    /// absolute address, which feeds name themselves by.
    pub(crate) fn xml(
        &mut self,
        at: Reserved,
        media: MediaType,
        write: impl FnOnce(&mut Xml, &str) -> io::Result<()>,
    ) -> Result<(), Error> {
        let route = at.route()?;
        let own = self.base.at(&route);
        let written = Xml::declared().and_then(|mut xml| write(&mut xml, &own).map(|()| xml.into()));
        let bytes = written.map_err(|source| Error::Xml { route: route.clone(), source })?;
        self.serve(Entry {
            route,
            media,
            status: Status::Ok,
            body: Body::Bytes(bytes),
            owner: Owner::Engine,
            cache: Cache::Revalidate,
        });
        Ok(())
    }

    /// Records a rule of the engine's own that did not hold.
    pub(crate) fn fail(&mut self, error: Error) {
        self.diagnostics.push(error.into());
    }

    /// The manifest of everything served, and everything said.
    pub(crate) fn finish(mut self) -> Rendered {
        trace::debug!(
            name: trace::event::RENDERED,
            reused = trace::Count(self.tally().0).field(),
            rendered = trace::Count(self.tally().1).field(),
        );
        let entries = self.entries.into_values().map(|(served, _)| served).collect();
        let manifest = Manifest::new(entries).unwrap_or_else(|duplicate| {
            self.diagnostics.push(Error::Served { route: duplicate.route }.into());
            Manifest::default()
        });
        Rendered {
            manifest,
            blobs: self.blobs,
            diagnostics: self.diagnostics,
            notices: self.notices,
            steps: self.steps,
        }
    }
}
