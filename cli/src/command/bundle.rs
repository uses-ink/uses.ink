//! The first-party components `ink` carries (decision 0054): `ink:format-markdown`,
//! `ink:format-obsidian`, `ink:theme-quiet` and `ink:source-git`, embedded by the build script
//! from what `cargo xtask component` built. The registry is generated, so a component added to
//! xtask's guest list is carried with no edit here.
//!
//! A folder given with `--components` can replace one by file name, per component; every
//! component a command runs says, as a notice, which bytes it used.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use model::{Announcement, Hash, Notice};
use serde::Serialize;

use crate::output::error::Error;

/// One embedded component.
#[derive(Debug)]
pub(crate) struct Entry {
    id: &'static str,
    version: &'static str,
    hash: Hash,
    bytes: &'static [u8],
}

impl Entry {
    /// An entry the generated registry lists.
    #[allow(dead_code, reason = "only the generated registry calls it, and it is empty in a build without components")]
    pub(crate) const fn new(id: &'static str, version: &'static str, hash: [u8; 32], bytes: &'static [u8]) -> Self {
        Self { id, version, hash: Hash::from_bytes(hash), bytes }
    }
}

/// The registry of what this `ink` was built with.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bundled;

impl Bundled {
    const ENTRIES: &'static [Entry] = include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

    /// Every component carried, in the order xtask lists the guests.
    #[cfg(test)]
    const fn all() -> &'static [Entry] {
        Self::ENTRIES
    }

    /// The component carried under `id`.
    pub(crate) fn entry(id: &str) -> Result<&'static Entry, Error> {
        Self::ENTRIES.iter().find(|entry| entry.id == id).ok_or_else(|| Error::Unbundled { id: id.to_owned() })
    }
}

/// Where a component's bytes came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    /// Carried in the binary.
    Bundled,
    /// A file of the folder `--components` names.
    Folder(PathBuf),
}

impl Origin {
    fn named(&self) -> String {
        match self {
            Self::Bundled => "bundled".to_owned(),
            Self::Folder(path) => path.display().to_string(),
        }
    }
}

/// A component to run: its bytes and where they came from.
#[derive(Debug)]
pub(crate) struct Held {
    id: &'static str,
    origin: Origin,
    /// The version of the bundled entry; unknown for a file of a folder.
    version: Option<&'static str>,
    hash: Hash,
    bytes: Cow<'static, [u8]>,
}

/// What a run says it used, in `--json`.
#[derive(Debug, Serialize)]
pub(crate) struct Used {
    id: &'static str,
    version: Option<&'static str>,
    hash: Hash,
    origin: String,
}

impl Held {
    /// The component's bytes.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The notice that says which bytes ran.
    pub(crate) fn notice(&self) -> Notice {
        Notice::new(Announcement::PluginComponentUsed {
            plugin: self.id.to_owned(),
            hash: self.hash.to_string(),
            origin: self.origin.named(),
        })
    }

    /// What it says of itself in `--json`.
    pub(crate) fn used(&self) -> Used {
        Used { id: self.id, version: self.version, hash: self.hash, origin: self.origin.named() }
    }
}

/// The components a command runs: the bundled ones, each replaced by a file of a folder when
/// the folder holds one.
#[derive(Clone, Debug)]
pub(crate) struct Components {
    folder: Option<PathBuf>,
}

impl Components {
    pub(crate) fn new(folder: Option<&Path>) -> Self {
        Self { folder: folder.map(Path::to_path_buf) }
    }

    /// The file a folder keeps `id` in, as `cargo xtask component` names it.
    fn file(id: &str) -> String {
        format!("{}.wasm", id.rsplit(':').next().unwrap_or(id))
    }

    /// The component `id`: the folder's file when there is one, else the bundled one.
    pub(crate) fn hold(&self, id: &'static str) -> Result<Held, Error> {
        if let Some(folder) = &self.folder {
            let path = folder.join(Self::file(id));
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let hash = Hash::of(&bytes);
                    return Ok(Held {
                        id,
                        origin: Origin::Folder(path),
                        version: None,
                        hash,
                        bytes: Cow::Owned(bytes),
                    });
                }
                Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(Error::Component { path, source }),
            }
        }
        let entry = Bundled::entry(id)?;
        Ok(Held {
            id,
            origin: Origin::Bundled,
            version: Some(entry.version),
            hash: entry.hash,
            bytes: Cow::Borrowed(entry.bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_party() {
        let ids: Vec<_> = Bundled::all().iter().map(|entry| entry.id).collect();
        let held = ["ink:format-markdown", "ink:format-obsidian", "ink:theme-quiet", "ink:source-git"];
        assert!(ids.is_empty() || ids == held, "{ids:?}");
    }

    #[test]
    fn hashes_hold() {
        for entry in Bundled::all() {
            assert_eq!(Hash::of(entry.bytes), entry.hash, "{}", entry.id);
        }
    }

    #[test]
    fn folder_replaces() {
        let dir = std::env::temp_dir().join(format!("ink-bundle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp folder");
        std::fs::write(dir.join("format-markdown.wasm"), b"not a component").expect("a file");
        let held = Components::new(Some(&dir)).hold("ink:format-markdown");
        std::fs::remove_dir_all(&dir).expect("cleaned");
        let held = held.expect("the folder's file is held without being parsed");
        assert_eq!(held.bytes(), b"not a component");
        assert!(matches!(held.origin, Origin::Folder(_)));
    }

    #[test]
    fn missing_falls_back() {
        let dir = std::env::temp_dir().join("ink-bundle-absent");
        let held = Components::new(Some(&dir)).hold("ink:theme-quiet");
        if Bundled::all().is_empty() {
            assert!(matches!(held, Err(Error::Unbundled { .. })));
        } else {
            assert_eq!(held.expect("the bundled one").origin, Origin::Bundled);
        }
    }
}
