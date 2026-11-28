//! What `ink build` wrote into `.ink-manifest.json`, read back.
//!
//! The file is [`model::Written`], with the parts the edge needs: the model's manifest types
//! serialize but do not deserialize, so the entries are read here and rebuilt. The CLI's test
//! serves a real build, so a drift in the file's shape fails there.

use std::path::{Component, Path};

use model::{Header, Manifest, Route, Served, Written};
use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::error::Error;

/// The file's name in a build's output folder.
pub const MANIFEST: &str = <Written>::MANIFEST;

/// The file as `ink build` wrote it, parsed and not yet checked: [`Unchecked::check`] says
/// whether it is a deploy.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Unchecked(Written<Vec<Listed>, Entries, IgnoredAny>);

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
#[non_exhaustive]
pub struct Checked {
    /// The routes, each with its blob, media type and headers.
    pub manifest: Manifest,
    /// Every file the build wrote: the hash of its bytes and where it sits.
    pub files: Vec<(model::Hash, model::ContentPath)>,
}

impl Unchecked {
    /// Checks everything the file says about itself; `path` is where it was read, for errors.
    pub fn check(self, path: &Path) -> Result<Checked, Error> {
        let Written { schema, deploy, files: listed, manifest, .. } = self.0;
        if schema != <Written>::SCHEMA {
            return Err(Error::Schema { path: path.to_path_buf(), found: schema });
        }
        let mut files = Vec::with_capacity(listed.len());
        for Listed { path: listed, hash } in listed {
            let listed =
                model::ContentPath::parse(listed).map_err(|source| Error::Path { path: path.to_path_buf(), source })?;
            if !Path::new(listed.as_str()).components().all(|part| matches!(part, Component::Normal(_))) {
                return Err(Error::Escapes { path: path.to_path_buf() });
            }
            files.push((hash, listed));
        }
        let entries = manifest
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
        if computed != deploy {
            return Err(Error::Id { path: path.to_path_buf(), recorded: deploy, computed });
        }
        Ok(Checked { manifest, files })
    }
}
