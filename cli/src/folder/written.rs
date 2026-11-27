//! The folder `ink build` writes into. What an earlier build left there is removed only when its
//! manifest lists it and its bytes still hash to what was written; nothing under the folder is
//! reached through a link, and nothing is written over a file no build wrote.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

use engine::Blobs;
use model::record::{Against, Build};
use model::{ContentPath, DeployId, Hash, Manifest, MediaType, PathError, Route, Written};
use serde::{Deserialize, Serialize};

use crate::output::error::Error;
use crate::output::failure::Remedy;

/// The manifest's file in the output folder. A dotted name is never a route, so no entry can
/// take it. The folder walk knows it by this name, so no command reads the folder as content.
const MANIFEST: &str = <Written>::MANIFEST;

/// What the next build reads back from [`Written`] to clear the folder.
#[derive(Debug, Deserialize)]
struct Previous {
    files: Vec<Listed>,
}

/// What the next build reads back from [`Written`] to compare with. The record is read on its
/// own, so a record this ink cannot read never stops a build from clearing the folder.
#[derive(Debug, Deserialize)]
struct Recorded {
    #[serde(default)]
    deploy: Option<DeployId>,
    #[serde(default)]
    record: Option<serde_json::Value>,
}

/// A deploy to write: its manifest, the bytes rendered for it and its record.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Deployed<'a> {
    pub(crate) manifest: &'a Manifest,
    pub(crate) blobs: &'a BTreeMap<Hash, Vec<u8>>,
    pub(crate) record: &'a Build,
}

/// What the previous build in the folder left to compare with.
#[derive(Debug)]
pub(crate) struct Baseline {
    pub(crate) against: Against,
    /// Its record, when it kept one this ink reads.
    pub(crate) record: Option<Build>,
    /// Why a record that was kept could not be read.
    pub(crate) unread: Option<serde_json::Error>,
}

/// One file a build wrote: where, and the hash of the bytes it wrote there.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Listed {
    path: Relative,
    hash: Hash,
}

/// A path inside the output folder: a [`ContentPath`] (no root, no `..`, no empty segment)
/// whose every component the platform reads as a plain name, so no prefix either.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub(crate) struct Relative(ContentPath);

/// Why text is no path inside the output folder.
#[derive(Debug)]
#[non_exhaustive]
pub enum RelativeError {
    /// It is no path of a site.
    Path(PathError),
    /// The platform reads a part of it as a root, a prefix or a step up.
    Component,
}

impl fmt::Display for RelativeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(_) => write!(out, "not a path inside the output folder"),
            Self::Component => write!(out, "a part of it leaves the output folder"),
        }
    }
}

impl std::error::Error for RelativeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(source) => Some(source),
            Self::Component => None,
        }
    }
}

impl TryFrom<String> for Relative {
    type Error = RelativeError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let path = ContentPath::parse(text).map_err(RelativeError::Path)?;
        if Path::new(path.as_str()).components().all(|component| matches!(component, Component::Normal(_))) {
            Ok(Self(path))
        } else {
            Err(RelativeError::Component)
        }
    }
}

impl From<Relative> for String {
    fn from(relative: Relative) -> Self {
        relative.0.into()
    }
}

impl AsRef<Path> for Relative {
    fn as_ref(&self) -> &Path {
        Path::new(self.0.as_str())
    }
}

/// An output folder `ink build` will not clear or write into. Each is refused before anything
/// in the folder changes.
#[derive(Debug)]
#[non_exhaustive]
pub enum Refusal {
    /// The folder holds files and no manifest of an earlier `ink build`.
    Foreign {
        /// The output folder.
        path: PathBuf,
    },
    /// The folder is the site's folder or holds it, so clearing it could reach sources.
    Holds {
        /// The output folder, as given.
        out: PathBuf,
        /// The site's folder, resolved.
        dir: PathBuf,
    },
    /// A link in the folder, where `ink build` writes none and follows none.
    Linked {
        /// The link.
        path: PathBuf,
    },
    /// A file the earlier build's manifest lists no longer holds the bytes it wrote.
    Changed {
        /// The file.
        path: PathBuf,
    },
    /// A file the build would write is already there, and no earlier build wrote it.
    Occupied {
        /// The file, or the file in the way of its folder.
        path: PathBuf,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foreign { path } => write!(out, "{} holds files no `ink build` wrote", path.display()),
            Self::Holds { out: folder, dir } => {
                write!(out, "the output folder {} is or holds the site's folder {}", folder.display(), dir.display())
            }
            Self::Linked { path } => {
                write!(out, "{} is a link, which `ink build` neither writes nor follows", path.display())
            }
            Self::Changed { path } => write!(out, "{} is not what the earlier `ink build` wrote there", path.display()),
            Self::Occupied { path } => write!(out, "{} is in the way, and no `ink build` wrote it", path.display()),
        }
    }
}

impl std::error::Error for Refusal {}

impl Refusal {
    /// What to do about it. Every refusal happens before anything is written.
    pub(crate) fn remedy(&self) -> Remedy {
        match self {
            Self::Holds { .. } | Self::Foreign { .. } => Remedy::ChooseOut,
            Self::Linked { .. } | Self::Changed { .. } | Self::Occupied { .. } => Remedy::MoveAside,
        }
    }

    /// Its kind, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Foreign { .. } => "output.folder.foreign",
            Self::Holds { .. } => "output.folder.source",
            Self::Linked { .. } => "output.file.linked",
            Self::Changed { .. } => "output.file.changed",
            Self::Occupied { .. } => "output.file.occupied",
        }
    }
}

impl From<Refusal> for Error {
    fn from(refusal: Refusal) -> Self {
        Self::Output(refusal)
    }
}

/// Where one entry is written: `/` is `index.html`, a page `/a/b` is `a/b/index.html`, the
/// not-found page is `404.html`, anything else its route.
#[derive(Clone, Copy, Debug)]
struct Placed<'a>(&'a Route, MediaType);

impl From<Placed<'_>> for String {
    fn from(Placed(route, media): Placed<'_>) -> Self {
        let route = route.as_str().trim_start_matches('/');
        match (media, route) {
            (MediaType::Html, "") => "index.html".to_owned(),
            (MediaType::Html, "404") => "404.html".to_owned(),
            (MediaType::Html, route) => format!("{route}/index.html"),
            (_, route) => route.to_owned(),
        }
    }
}

impl Relative {
    /// `file` as a path in the output folder, or the error saying why it is none.
    fn place(file: String) -> Result<Self, Error> {
        Self::try_from(file.clone()).map_err(|source| Error::Placed { file, source })
    }

    /// Each path from the first part of this one to the whole of it: `a`, `a/b`, `a/b/c`.
    fn prefixes(&self) -> impl Iterator<Item = &str> {
        let text = self.0.as_str();
        text.match_indices('/').filter_map(|(at, _)| text.get(..at)).chain(std::iter::once(text))
    }
}

/// The folder a build writes into, resolved: its links followed up to the last part that exists.
#[derive(Debug)]
pub(crate) struct OutFolder {
    root: PathBuf,
    /// [`MANIFEST`] as a path in it.
    manifest: Relative,
}

impl OutFolder {
    /// The output folder `root` for the site's folder `dir`, refused when it is `dir` or holds
    /// it: clearing it could reach the site's own files. A `dir` that cannot be listed is refused
    /// as the folder walk would refuse it.
    pub(crate) fn new(root: &Path, dir: &Path) -> Result<Self, Error> {
        let refused = |source| Error::Refused(host_native::Refused::Root { path: dir.to_path_buf(), source });
        std::fs::read_dir(dir).map_err(refused)?;
        let dir = std::fs::canonicalize(dir).map_err(refused)?;
        let resolved = Resolved::of(root)?;
        if dir.starts_with(&resolved) {
            return Err(Refusal::Holds { out: root.to_path_buf(), dir }.into());
        }
        Ok(Self { root: resolved, manifest: Relative::place(MANIFEST.to_owned())? })
    }

    /// The output folder as a path in the site's folder `dir`, when it is inside it.
    pub(crate) fn inside(&self, dir: &Path) -> Option<ContentPath> {
        let dir = std::fs::canonicalize(dir).ok()?;
        let relative = self.root.strip_prefix(&dir).ok()?.to_str()?;
        relative.parse().ok()
    }

    /// Refuses the folder now, before the site is built, for what [`OutFolder::replace`] would
    /// refuse in it: no manifest beside other files, a manifest that does not parse, or a listed
    /// file changed or reached through a link. `replace` checks again before it removes anything.
    pub(crate) fn check(&self) -> Result<(), Error> {
        for listed in &self.previous()? {
            self.verify(listed)?;
        }
        Ok(())
    }

    /// Replaces what the earlier build wrote with `manifest`'s entries, then writes the manifest.
    /// Every check runs before anything is removed: a refusal leaves the folder as it was.
    pub(crate) fn replace(&self, deployed: Deployed<'_>, store: &impl Blobs) -> Result<(), Error> {
        let Deployed { manifest, blobs, record } = deployed;
        let files = Files::of(manifest, blobs, store)?;
        let previous = self.previous()?;
        let ours: BTreeSet<&Relative> = previous.iter().map(|listed| &listed.path).collect();
        for listed in &previous {
            self.verify(listed)?;
        }
        for path in files.0.keys() {
            Under(&self.root).free(path, &ours)?;
        }
        for listed in &previous {
            Under(&self.root).remove(&listed.path)?;
        }
        let listed: Vec<Listed> =
            files.0.iter().map(|(path, bytes)| Listed { path: path.clone(), hash: Hash::of(bytes) }).collect();
        for (path, bytes) in &files.0 {
            Under(&self.root).put(path, bytes)?;
        }
        let written = Written::new(manifest.id(), listed.as_slice(), manifest, record);
        let json = serde_json::to_vec(&written).map_err(|source| Error::Json { source })?;
        Under(&self.root).put(&self.manifest, &json)
    }

    /// What the previous build left to compare with: its deploy and record, or nothing when
    /// the folder holds no build. Call after [`OutFolder::check`], which refuses what is not
    /// a build's folder.
    pub(crate) fn baseline(&self) -> Result<Baseline, Error> {
        let nothing = Baseline { against: Against::Nothing, record: None, unread: None };
        if !self.root.join(MANIFEST).is_file() {
            return Ok(nothing);
        }
        let bytes = Under(&self.root).bytes(&self.manifest)?;
        let path = self.root.join(MANIFEST);
        let recorded: Recorded = serde_json::from_slice(&bytes).map_err(|source| Error::Manifest { path, source })?;
        let deploy = recorded.deploy;
        let unrecorded = Against::Unrecorded { deploy };
        Ok(match recorded.record.map(serde_json::from_value::<Build>) {
            None => Baseline { against: unrecorded, ..nothing },
            Some(Ok(record)) => match deploy {
                Some(deploy) => Baseline { against: Against::Deploy { deploy }, record: Some(record), unread: None },
                None => Baseline { against: unrecorded, ..nothing },
            },
            Some(Err(error)) => Baseline { against: unrecorded, record: None, unread: Some(error) },
        })
    }

    /// The folder itself.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// What the earlier build listed, its manifest last; nothing when the folder is new or
    /// empty. A folder holding files and no manifest is refused.
    fn previous(&self) -> Result<Vec<Listed>, Error> {
        let manifest = self.root.join(MANIFEST);
        match std::fs::symlink_metadata(&manifest) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(metadata) if metadata.is_symlink() => return Err(Refusal::Linked { path: manifest }.into()),
            Ok(_) => return Err(Refusal::Changed { path: manifest }.into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return if self.is_empty()? {
                    Ok(Vec::new())
                } else {
                    Err(Refusal::Foreign { path: self.root.clone() }.into())
                };
            }
            Err(source) => return Err(Error::Read { path: manifest, source }),
        }
        let bytes = Under(&self.root).bytes(&self.manifest)?;
        let previous: Previous =
            serde_json::from_slice(&bytes).map_err(|source| Error::Manifest { path: manifest, source })?;
        let mut files = previous.files;
        files.push(Listed { path: self.manifest.clone(), hash: Hash::of(&bytes) });
        Ok(files)
    }

    /// Whether the folder is missing or holds nothing.
    fn is_empty(&self) -> Result<bool, Error> {
        let read = |source| Error::Read { path: self.root.clone(), source };
        match std::fs::read_dir(&self.root) {
            Ok(mut entries) => Ok(entries.next().transpose().map_err(read)?.is_none()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(source) => Err(read(source)),
        }
    }

    /// Refuses `listed` unless it is gone or still holds the bytes the earlier build wrote.
    fn verify(&self, listed: &Listed) -> Result<(), Error> {
        let under = Under(&self.root);
        match under.kind(&listed.path)? {
            Kind::Missing => Ok(()),
            Kind::File if Hash::of(under.bytes(&listed.path)?) == listed.hash => Ok(()),
            Kind::Folder | Kind::File | Kind::Other => {
                Err(Refusal::Changed { path: self.root.join(&listed.path) }.into())
            }
        }
    }
}

/// The files of a deploy, each with its bytes, by where they go.
#[derive(Debug)]
struct Files<'a>(BTreeMap<Relative, std::borrow::Cow<'a, [u8]>>);

impl<'a> Files<'a> {
    fn of(manifest: &Manifest, blobs: &'a BTreeMap<Hash, Vec<u8>>, store: &'a impl Blobs) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        for entry in manifest.entries() {
            let path = Relative::place(Placed(entry.route(), entry.media()).into())?;
            let hash = entry.blob();
            let bytes = match blobs.get(&hash) {
                Some(bytes) => std::borrow::Cow::Borrowed(bytes.as_slice()),
                None => store.read(&hash).map_err(|source| Error::Blob { route: entry.route().clone(), source })?,
            };
            if files.insert(path.clone(), bytes).is_some() {
                return Err(Refusal::Occupied { path: PathBuf::from(String::from(path)) }.into());
            }
        }
        Ok(Self(files))
    }
}

/// What one path under the output folder is, its links not followed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Missing,
    Folder,
    File,
    /// A device, a pipe, a socket.
    Other,
}

/// Paths under the output folder, reached one part at a time, refusing every link.
#[derive(Clone, Copy, Debug)]
struct Under<'a>(&'a Path);

impl Under<'_> {
    /// What the one path `part` is; a link is refused.
    fn step(self, part: &str) -> Result<Kind, Error> {
        let path = self.0.join(part);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() => Err(Refusal::Linked { path }.into()),
            Ok(metadata) if metadata.is_dir() => Ok(Kind::Folder),
            Ok(metadata) if metadata.is_file() => Ok(Kind::File),
            Ok(_) => Ok(Kind::Other),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Kind::Missing),
            Err(source) => Err(Error::Read { path, source }),
        }
    }

    /// What `relative` is: missing when a part on the way is not a folder. A link on the way to
    /// it, or at it, is refused.
    fn kind(self, relative: &Relative) -> Result<Kind, Error> {
        let mut kind = Kind::Folder;
        for part in relative.prefixes() {
            if kind != Kind::Folder {
                return Ok(Kind::Missing);
            }
            kind = self.step(part)?;
        }
        Ok(kind)
    }

    /// The bytes of the file at `relative`, read through one handle.
    fn bytes(self, relative: &Relative) -> Result<Vec<u8>, Error> {
        let path = self.0.join(relative);
        let read = |source| Error::Read { path: path.clone(), source };
        let mut file = std::fs::File::open(&path).map_err(read)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(read)?;
        Ok(bytes)
    }

    /// Refuses writing `relative` when a file no build wrote is on the way or there, or a link.
    fn free(self, relative: &Relative, ours: &BTreeSet<&Relative>) -> Result<(), Error> {
        for part in relative.prefixes() {
            let is_ours = || ours.iter().any(|listed| listed.0.as_str() == part);
            match self.step(part)? {
                Kind::Missing => return Ok(()),
                Kind::Folder if part != relative.0.as_str() => {}
                Kind::File if is_ours() => return Ok(()),
                Kind::Folder | Kind::File | Kind::Other => {
                    return Err(Refusal::Occupied { path: self.0.join(part) }.into());
                }
            }
        }
        Ok(())
    }

    /// Removes the file at `relative`, checked by [`OutFolder::verify`], when it is still there.
    fn remove(self, relative: &Relative) -> Result<(), Error> {
        if self.kind(relative)? == Kind::Missing {
            return Ok(());
        }
        let path = self.0.join(relative);
        std::fs::remove_file(&path).map_err(|source| Error::Write { path, source })
    }

    /// Writes `bytes` to a new file at `relative`, making each folder on the way and refusing
    /// a link or a file already there.
    fn put(self, relative: &Relative, bytes: &[u8]) -> Result<(), Error> {
        let root = self.0;
        std::fs::create_dir_all(root).map_err(|source| Error::Write { path: root.to_path_buf(), source })?;
        let mut parts = relative.prefixes().peekable();
        while let Some(part) = parts.next() {
            let path = root.join(part);
            if parts.peek().is_none() {
                let write = |source| Error::Write { path: path.clone(), source };
                let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(write)?;
                return file.write_all(bytes).map_err(write);
            }
            match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if self.step(part)? != Kind::Folder {
                        return Err(Refusal::Occupied { path }.into());
                    }
                }
                Err(source) => return Err(Error::Write { path, source }),
            }
        }
        Ok(())
    }
}

/// A path that may not exist yet, its existing part resolved through its links.
#[derive(Clone, Copy, Debug)]
struct Resolved;

impl Resolved {
    /// `path` made absolute, its longest existing ancestor canonical and the rest appended.
    fn of(path: &Path) -> Result<PathBuf, Error> {
        let read = |source| Error::Read { path: path.to_path_buf(), source };
        let absolute = std::path::absolute(path).map_err(read)?;
        let mut rest = Vec::new();
        let mut existing = absolute.as_path();
        let base = loop {
            match std::fs::canonicalize(existing) {
                Ok(base) => break base,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    rest.push(existing.file_name().map(ToOwned::to_owned));
                    existing = existing.parent().ok_or_else(|| read(error))?;
                }
                Err(source) => return Err(read(source)),
            }
        };
        let mut resolved = base;
        for part in rest.into_iter().rev() {
            match part {
                Some(name) => resolved.push(name),
                None => {
                    resolved.pop();
                }
            }
        }
        Ok(resolved)
    }
}
