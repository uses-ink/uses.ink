//! `ink build` and `ink check`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use engine::Blobs;
use model::{ContentPath, DeployId, Hash, Manifest, MediaType, Route};
use serde::{Deserialize, Serialize};

use crate::args::{Build, Check};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::{Address, DEFAULT_OUT, Source};

/// The manifest's file in the output folder. A dotted name is never a route, so no entry can
/// take it.
const MANIFEST: &str = ".ink-manifest.json";

/// What `ink build --json` prints.
#[derive(Debug, Serialize)]
struct Built<'a> {
    schema: &'static str,
    deploy: DeployId,
    out: &'a Path,
    entries: usize,
}

/// The manifest as written into the output folder.
#[derive(Debug, Serialize)]
struct Written<'a> {
    schema: &'static str,
    deploy: DeployId,
    /// Every file written, relative to the output folder, so the next build can remove them.
    files: Vec<String>,
    manifest: &'a Manifest,
}

/// What the next build reads back from [`Written`].
#[derive(Debug, Deserialize)]
struct Previous {
    files: Vec<String>,
}

/// What `ink check --json` prints.
#[derive(Debug, Serialize)]
struct Checked {
    schema: &'static str,
    pages: usize,
    decisions: usize,
    errors: usize,
    warnings: usize,
}

impl Build {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let address = Address::from_flag(self.base.as_deref())?;
        let out = self.out.clone().unwrap_or_else(|| self.dir.join(DEFAULT_OUT));
        let folder = OutFolder::prepare(&out)?;
        let mut source = Source::new(&self.dir);
        source.output.extend(folder.inside(&self.dir).filter(|inside| !source.output.contains(inside)));
        let loaded = source.load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let rendered = loaded.render(&address);
        let tally = io.said(&rendered.diagnostics, &rendered.notices, texts)?;
        let Some(deploy) = rendered.deploy.as_ref().filter(|_| tally.errors == 0) else {
            return Ok(Exit::Content);
        };
        folder.write(deploy, &rendered.blobs, &loaded.store)?;
        let id = deploy.id();
        io.line(id)?;
        io.json(&Built { schema: "uses.build/v1", deploy: id, out: &out, entries: deploy.entries().len() })?;
        Ok(tally.exit(Strict::from(self.strict)))
    }
}

impl Check {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let loaded = Source::new(&self.dir).load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let pages = understood.site.as_ref().map_or(0, |site| site.pages().len());
        let decisions = understood.decisions.len();
        io.line(format_args!(
            "{pages} pages, {decisions} decisions, {} errors, {} warnings",
            tally.errors, tally.warnings
        ))?;
        io.json(&Checked {
            schema: "uses.check/v1",
            pages,
            decisions,
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        Ok(tally.exit(Strict::from(self.strict)))
    }
}

/// The folder a build writes into.
#[derive(Debug)]
struct OutFolder {
    root: PathBuf,
}

/// Where one entry is written: `/` is `index.html`, a page `/a/b` is `a/b/index.html`, the
/// not-found page is `404.html`, anything else its route.
#[derive(Clone, Copy, Debug)]
struct Placed<'a>(&'a Route, MediaType);

impl Placed<'_> {
    fn file(self) -> String {
        let route = self.0.as_str().trim_start_matches('/');
        match (self.1, route) {
            (MediaType::Html, "") => "index.html".to_owned(),
            (MediaType::Html, "404") => "404.html".to_owned(),
            (MediaType::Html, route) => format!("{route}/index.html"),
            (_, route) => route.to_owned(),
        }
    }
}

impl OutFolder {
    /// Makes the folder, and clears what an earlier build wrote there. A folder holding files no
    /// build wrote is refused, never overwritten.
    fn prepare(root: &Path) -> Result<Self, Error> {
        let write = |source| Error::Write { path: root.to_path_buf(), source };
        std::fs::create_dir_all(root).map_err(write)?;
        let manifest = root.join(MANIFEST);
        if manifest.is_file() {
            let text =
                std::fs::read_to_string(&manifest).map_err(|source| Error::Write { path: manifest.clone(), source })?;
            let previous: Previous =
                serde_json::from_str(&text).map_err(|source| Error::Manifest { path: manifest.clone(), source })?;
            for file in previous.files.iter().chain([&MANIFEST.to_owned()]) {
                let path = root.join(file);
                if path.is_file() {
                    std::fs::remove_file(&path).map_err(|source| Error::Write { path, source })?;
                }
            }
        } else if std::fs::read_dir(root).map_err(write)?.next().is_some() {
            return Err(Error::Foreign { path: root.to_path_buf() });
        }
        Ok(Self { root: root.to_path_buf() })
    }

    /// The output folder as a path in the site's folder `dir`, when it is inside it.
    fn inside(&self, dir: &Path) -> Option<ContentPath> {
        let dir = std::fs::canonicalize(dir).ok()?;
        let root = std::fs::canonicalize(&self.root).ok()?;
        let relative = root.strip_prefix(&dir).ok()?.to_str()?;
        relative.parse().ok()
    }

    /// Every entry of `manifest`, then the manifest itself.
    fn write(&self, manifest: &Manifest, blobs: &BTreeMap<Hash, Vec<u8>>, store: &impl Blobs) -> Result<(), Error> {
        let mut files = Vec::new();
        for entry in manifest.entries() {
            let file = Placed(entry.route(), entry.media()).file();
            let hash = entry.blob();
            let bytes = match blobs.get(&hash) {
                Some(bytes) => std::borrow::Cow::Borrowed(bytes.as_slice()),
                None => store.read(&hash).map_err(|missing| Error::Write {
                    path: self.root.join(&file),
                    source: std::io::Error::new(std::io::ErrorKind::NotFound, missing),
                })?,
            };
            self.put(&file, &bytes)?;
            files.push(file);
        }
        let written = Written { schema: "uses.manifest/v1", deploy: manifest.id(), files, manifest };
        let json = serde_json::to_vec_pretty(&written).map_err(|source| Error::Json { source })?;
        self.put(MANIFEST, &json)
    }

    fn put(&self, file: &str, bytes: &[u8]) -> Result<(), Error> {
        let path = self.root.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Write { path: parent.to_path_buf(), source })?;
        }
        std::fs::write(&path, bytes).map_err(|source| Error::Write { path, source })
    }
}
