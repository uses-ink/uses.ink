//! Host to deploy over folders: [`Deploys`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use edge::{Deploy, Hostname, Resolved, Sites};
use model::ContentPath;
use model::Hash;

use crate::error::Error;
use crate::written::{MANIFEST, Written};

/// How hosts map to folders.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// One deploy when the folder holds a manifest, else one folder per host.
    #[default]
    Auto,
    /// The folder is one deploy, served for every host name.
    Single,
    /// `<folder>/<host>` is the deploy of `<host>`; a host with no folder is no site.
    Folders,
}

/// The files of one deploy: where it sits and which file holds each blob. The scope [`Files`](crate::Files)
/// reads in.
#[derive(Debug)]
pub struct Placed {
    pub(crate) root: PathBuf,
    pub(crate) files: BTreeMap<Hash, ContentPath>,
}

/// When a manifest was last read: a change of either number reads it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    length: u64,
}

#[derive(Debug)]
struct Cached {
    stamp: Stamp,
    deploy: Arc<Deploy>,
    placed: Arc<Placed>,
}

/// The deploys of a folder, by host.
#[derive(Debug)]
pub struct Deploys {
    root: PathBuf,
    mode: Mode,
    cache: Mutex<BTreeMap<PathBuf, Cached>>,
}

impl Deploys {
    /// The deploys under `root`. [`Mode::Auto`] is decided here, once, from whether `root` holds
    /// a manifest, and [`Mode::Single`] refuses a folder that holds none.
    pub fn new(root: impl Into<PathBuf>, mode: Mode) -> Result<Self, Error> {
        let root = root.into();
        let meta = std::fs::metadata(&root).map_err(|source| Error::Read { path: root.clone(), source })?;
        if !meta.is_dir() {
            return Err(Error::NotFolder { root });
        }
        let holds = std::fs::symlink_metadata(root.join(MANIFEST)).is_ok_and(|meta| meta.is_file());
        let mode = match (mode, holds) {
            (Mode::Auto, true) | (Mode::Single, true) => Mode::Single,
            (Mode::Auto, false) | (Mode::Folders, _) => Mode::Folders,
            (Mode::Single, false) => return Err(Error::NotDeploy { root }),
            (mode, _) => mode,
        };
        Ok(Self { root, mode, cache: Mutex::new(BTreeMap::new()) })
    }

    /// How hosts are mapped, [`Mode::Auto`] decided: [`Mode::Single`] or [`Mode::Folders`].
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The folder `hostname`'s deploy sits in, when it has one by name.
    fn folder(&self, hostname: &Hostname) -> Option<PathBuf> {
        match self.mode {
            Mode::Folders => hostname.name().map(|name| self.root.join(name)),
            _ => Some(self.root.clone()),
        }
    }

    /// The deploy in `folder`, read now or taken from the cache when its manifest is unchanged.
    async fn load(&self, folder: PathBuf) -> Result<Option<Resolved<Arc<Placed>>>, Error> {
        let path = folder.join(MANIFEST);
        let meta = match tokio::fs::symlink_metadata(&path).await {
            Ok(meta) => meta,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Read { path, source }),
        };
        if meta.is_symlink() {
            return Err(Error::Linked { path });
        }
        if !meta.is_file() {
            return Err(Error::NotFile { path });
        }
        let stamp = Stamp { modified: meta.modified().ok(), length: meta.len() };
        if let Some(cached) =
            self.cache.lock().unwrap_or_else(PoisonError::into_inner).get(&folder).filter(|c| c.stamp == stamp)
        {
            return Ok(Some(Resolved { deploy: Arc::clone(&cached.deploy), scope: Arc::clone(&cached.placed) }));
        }
        let bytes = tokio::fs::read(&path).await.map_err(|source| Error::Read { path: path.clone(), source })?;
        let written: Written =
            serde_json::from_slice(&bytes).map_err(|source| Error::Manifest { path: path.clone(), source })?;
        let checked = written.check(&path)?;
        let files: BTreeMap<Hash, ContentPath> = checked.files.into_iter().rev().collect();
        if let Some(entry) = checked.manifest.entries().iter().find(|entry| !files.contains_key(&entry.blob())) {
            return Err(Error::Unplaced { route: entry.route().clone() });
        }
        let deploy = Arc::new(Deploy { manifest: checked.manifest, ..Deploy::default() });
        let placed = Arc::new(Placed { root: folder.clone(), files });
        let resolved = Resolved { deploy: Arc::clone(&deploy), scope: Arc::clone(&placed) };
        self.cache.lock().unwrap_or_else(PoisonError::into_inner).insert(folder, Cached { stamp, deploy, placed });
        Ok(Some(resolved))
    }
}

impl Sites for Deploys {
    type Scope = Arc<Placed>;
    type Error = Error;

    async fn resolve(&self, hostname: &Hostname) -> Result<Option<Resolved<Arc<Placed>>>, Error> {
        match self.folder(hostname) {
            Some(folder) => self.load(folder).await,
            None => Ok(None),
        }
    }
}
