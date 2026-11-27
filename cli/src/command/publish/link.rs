//! The link file: `.uses/site` in the site's folder says which site the folder is published as
//! and where that site's API is. It holds no credential: a key is `ink login`'s, in the config
//! folder, never in the folder that is shared (`docs/cli.md`, decision 0077).

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::deploy::Failure;

/// The schema a link file is written under.
const SCHEMA: &str = "uses.link/v1";

/// Why a link file cannot be read or written.
#[derive(Debug)]
#[non_exhaustive]
pub enum LinkError {
    /// The file exists and cannot be read.
    Read(std::io::Error),
    /// The file is not the TOML of a link.
    Parse(toml::de::Error),
    /// The file is of another schema.
    Schema {
        /// The schema it names.
        found: String,
    },
    /// The link could not be written as TOML.
    Encode(toml::ser::Error),
    /// The file or its folder could not be written.
    Write(std::io::Error),
}

impl fmt::Display for LinkError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => out.write_str("the file could not be read"),
            Self::Parse(_) => out.write_str("the file is not a link"),
            Self::Schema { found } => write!(out, "the file is of the schema {found:?}, not {SCHEMA:?}"),
            Self::Encode(_) => out.write_str("the link could not be written as TOML"),
            Self::Write(_) => out.write_str("the file could not be written"),
        }
    }
}

impl std::error::Error for LinkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) | Self::Write(source) => Some(source),
            Self::Parse(source) => Some(source),
            Self::Encode(source) => Some(source),
            Self::Schema { .. } => None,
        }
    }
}

/// What a folder is linked to.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Link {
    schema: String,
    /// The API's address.
    pub(crate) api: String,
    /// The site's id.
    pub(crate) site: String,
    /// The site's handle when it was linked, for a person reading the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) handle: Option<String>,
}

impl Link {
    /// The file, relative to the site's folder.
    pub(crate) const FILE: &'static str = ".uses/site";

    /// A link to `site` at `api`.
    pub(crate) fn new(api: String, site: String, handle: Option<String>) -> Self {
        Self { schema: SCHEMA.to_owned(), api, site, handle }
    }

    /// Where the file is, for the folder `dir`.
    pub(crate) fn path(dir: &Path) -> PathBuf {
        dir.join(Self::FILE)
    }

    /// The folder's link, or none when it has no link file.
    pub(crate) fn find(dir: &Path) -> Result<Option<Self>, Failure> {
        let path = Self::path(dir);
        let unusable = |source| Failure::Link { path: path.clone(), source };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(unusable(LinkError::Read(error))),
        };
        let link: Self = toml::from_str(&text).map_err(|error| unusable(LinkError::Parse(error)))?;
        if link.schema == SCHEMA { Ok(Some(link)) } else { Err(unusable(LinkError::Schema { found: link.schema })) }
    }

    /// Writes the link into the folder `dir`: to a file beside it first, then renamed into place,
    /// so a stopped write leaves the old link or none. Returns the file.
    pub(crate) fn save(&self, dir: &Path) -> Result<PathBuf, Failure> {
        let path = Self::path(dir);
        let unusable = |source| Failure::Link { path: path.clone(), source };
        let text = toml::to_string_pretty(self).map_err(|error| unusable(LinkError::Encode(error)))?;
        let partial = path.with_extension("partial");
        let folder = path.parent().unwrap_or(dir);
        std::fs::create_dir_all(folder).map_err(|error| unusable(LinkError::Write(error)))?;
        std::fs::write(&partial, text).map_err(|error| unusable(LinkError::Write(error)))?;
        std::fs::rename(&partial, &path).map_err(|error| unusable(LinkError::Write(error)))?;
        Ok(path)
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use super::*;

    #[test]
    fn a_link_is_written_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Link::find(dir.path()).unwrap(), None);
        let link = Link::new("http://api.localhost:8080".into(), "ab".repeat(16), Some("garden".into()));
        let path = link.save(dir.path()).unwrap();
        assert!(path.ends_with(".uses/site"));
        assert_eq!(Link::find(dir.path()).unwrap(), Some(link));
        assert!(!dir.path().join(".uses/site.partial").exists());
    }

    #[test]
    fn a_file_that_is_no_link_is_refused_with_its_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".uses")).unwrap();
        std::fs::write(Link::path(dir.path()), "site = 3\n").unwrap();
        let failure = Link::find(dir.path()).unwrap_err();
        assert_eq!(failure.kind(), "link.file.unusable");
        std::fs::write(Link::path(dir.path()), "schema = \"x\"\napi = \"a\"\nsite = \"b\"\n").unwrap();
        assert!(matches!(Link::find(dir.path()), Err(Failure::Link { source: LinkError::Schema { .. }, .. })));
    }
}
