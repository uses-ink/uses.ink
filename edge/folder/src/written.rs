//! What `ink build` wrote into `.ink-manifest.json`, read back.
//!
//! These types mirror `cli/src/out.rs`'s `Written` for the fields the edge needs: the model's
//! manifest types serialize but do not deserialize. The CLI's test serves a real build, so a
//! drift in the file's shape fails there.

use std::path::{Component, Path};

use model::{DeployId, Header, Manifest, Route, Served};
use serde::Deserialize;

use crate::error::Error;

/// The file's name in a build's output folder.
pub(crate) const MANIFEST: &str = ".ink-manifest.json";

/// The schema this reads.
const SCHEMA: &str = "uses.manifest/v3";

#[derive(Debug, Deserialize)]
pub(crate) struct Written {
    schema: String,
    deploy: DeployId,
    files: Vec<Listed>,
    manifest: Entries,
}

#[derive(Debug, Deserialize)]
struct Listed {
    path: String,
    hash: model::Hash,
}

#[derive(Debug, Deserialize)]
struct Entries {
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    route: Route,
    blob: model::Hash,
    media: model::MediaType,
    status: model::Status,
    headers: Vec<Line>,
}

#[derive(Debug, Deserialize)]
struct Line {
    name: model::HeaderName,
    value: model::HeaderValue,
}

/// A manifest checked: its schema, its id, its files, its routes.
#[derive(Debug)]
pub(crate) struct Checked {
    pub(crate) manifest: Manifest,
    pub(crate) files: Vec<(model::Hash, model::ContentPath)>,
}

impl Written {
    /// Checks everything the file says about itself; `path` is where it was read, for errors.
    pub(crate) fn check(self, path: &Path) -> Result<Checked, Error> {
        if self.schema != SCHEMA {
            return Err(Error::Schema { path: path.to_path_buf(), found: self.schema });
        }
        let mut files = Vec::with_capacity(self.files.len());
        for Listed { path: listed, hash } in self.files {
            let listed =
                model::ContentPath::parse(listed).map_err(|source| Error::Path { path: path.to_path_buf(), source })?;
            if !Path::new(listed.as_str()).components().all(|part| matches!(part, Component::Normal(_))) {
                return Err(Error::Escapes { path: path.to_path_buf() });
            }
            files.push((hash, listed));
        }
        let entries = self
            .manifest
            .entries
            .into_iter()
            .map(|Entry { route, blob, media, status, headers }| {
                let headers = headers.into_iter().map(|Line { name, value }| Header::new(name, value)).collect();
                Served::new(route, blob, media, headers).with_status(status)
            })
            .collect();
        let manifest =
            Manifest::new(entries).map_err(|source| Error::Duplicate { path: path.to_path_buf(), source })?;
        let computed = manifest.id();
        if computed != self.deploy {
            return Err(Error::Id { path: path.to_path_buf(), recorded: self.deploy, computed });
        }
        Ok(Checked { manifest, files })
    }
}
