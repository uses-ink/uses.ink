//! What a render has produced so far: one entry per route, the bytes behind new blobs, and what
//! was said along the way.

use std::collections::BTreeMap;

use model::{ContentPath, Diagnostic, Hash, Manifest, MediaType, Notice, Route, Served, Status};

use crate::error::Broken;
use crate::render::Rendered;
use crate::render::header::Headers;

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
}

#[derive(Debug)]
pub(crate) struct Output {
    headers: Headers,
    entries: BTreeMap<Route, (Served, Owner)>,
    blobs: BTreeMap<Hash, Vec<u8>>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) notices: Vec<Notice>,
}

impl Output {
    pub(crate) fn new(headers: Headers) -> Self {
        Self { headers, entries: BTreeMap::new(), blobs: BTreeMap::new(), diagnostics: Vec::new(), notices: Vec::new() }
    }

    /// Who already serves `route`.
    pub(crate) fn owner(&self, route: &Route) -> Option<&Owner> {
        self.entries.get(route).map(|(_, owner)| owner)
    }

    /// Serves `entry` with the headers of its media type. A route served twice is a bug: the
    /// first entry stays.
    pub(crate) fn serve(&mut self, entry: Entry) {
        if self.entries.contains_key(&entry.route) {
            self.diagnose(Broken::RouteRepeated { route: entry.route }.into());
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
        let served = Served::new(entry.route.clone(), blob, entry.media, self.headers.of(entry.media));
        self.entries.insert(entry.route, (served.with_status(entry.status), entry.owner));
    }

    pub(crate) fn diagnose(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Records the failure of a step, if it failed.
    pub(crate) fn settle(&mut self, step: Result<(), Broken>) {
        if let Err(broken) = step {
            self.diagnose(broken.into());
        }
    }

    pub(crate) fn finish(mut self) -> Rendered {
        let entries = self.entries.into_values().map(|(served, _)| served).collect();
        let manifest = match Manifest::new(entries) {
            Ok(manifest) => manifest,
            Err(duplicate) => {
                self.diagnostics.push(Broken::RouteRepeated { route: duplicate.route }.into());
                Manifest::default()
            }
        };
        Rendered { manifest, blobs: self.blobs, diagnostics: self.diagnostics, notices: self.notices }
    }
}
