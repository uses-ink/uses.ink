//! Host to deploy over folders: [`Deploys`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use logging::Field as _;
use tokio::io::AsyncReadExt as _;

use edge::{Deploy, Hostname, Resolved, Sites};
use model::ContentPath;
use model::Hash;

use crate::error::Error;
use crate::written::{MANIFEST, Unchecked};

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

impl Placed {
    /// A deploy whose blobs are files under `root`, each at the path `files` gives its hash.
    pub const fn new(root: PathBuf, files: BTreeMap<Hash, ContentPath>) -> Self {
        Self { root, files }
    }
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
    /// The most bytes of a manifest the edge reads: 16 MiB, far over what a site of tens of
    /// thousands of pages writes. A larger manifest is [`Error::Large`], so a corrupt or hostile
    /// one is never held in memory.
    pub const MANIFEST_LIMIT: u64 = 16 * 1024 * 1024;

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
            (Mode::Auto | Mode::Single, true) => Mode::Single,
            (Mode::Auto, false) | (Mode::Folders, _) => Mode::Folders,
            (Mode::Single, false) => return Err(Error::NotDeploy { root }),
        };
        logging::debug!(
            name: logging::event::FOLDER_OPENED,
            root = logging::Located(&root).field(),
            mode = if mode == Mode::Single { "single" } else { "folders" },
        );
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

    /// The manifest's bytes, at most [`Deploys::MANIFEST_LIMIT`] of them.
    async fn slurp(path: &std::path::Path) -> Result<Vec<u8>, Error> {
        let read = |source| Error::Read { path: path.to_path_buf(), source };
        let mut bytes = Vec::new();
        let file = tokio::fs::File::open(path).await.map_err(read)?;
        file.take(Self::MANIFEST_LIMIT + 1).read_to_end(&mut bytes).await.map_err(read)?;
        if bytes.len() as u64 > Self::MANIFEST_LIMIT {
            return Err(Error::Large { path: path.to_path_buf(), limit: Self::MANIFEST_LIMIT });
        }
        Ok(bytes)
    }

    /// Refuses a site folder that is a link, in the mode where a request names it.
    async fn unlinked(&self, folder: &std::path::Path) -> Result<(), Error> {
        if self.mode != Mode::Folders {
            return Ok(());
        }
        match tokio::fs::symlink_metadata(folder).await {
            Ok(meta) if meta.is_symlink() => Err(Error::Linked { path: folder.to_path_buf() }),
            Ok(_) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(Error::Read { path: folder.to_path_buf(), source }),
        }
    }

    /// When the manifest at `path` was last changed, or `None` when there is none; a link, a
    /// thing that is no file and a file over the limit are refused.
    async fn stamped(path: &std::path::Path) -> Result<Option<Stamp>, Error> {
        let meta = match tokio::fs::symlink_metadata(path).await {
            Ok(meta) => meta,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Read { path: path.to_path_buf(), source }),
        };
        if meta.is_symlink() {
            return Err(Error::Linked { path: path.to_path_buf() });
        }
        if !meta.is_file() {
            return Err(Error::NotFile { path: path.to_path_buf() });
        }
        if meta.len() > Self::MANIFEST_LIMIT {
            return Err(Error::Large { path: path.to_path_buf(), limit: Self::MANIFEST_LIMIT });
        }
        Ok(Some(Stamp { modified: meta.modified().ok(), length: meta.len() }))
    }

    /// The deploy in `folder`, read now or taken from the cache when its manifest is unchanged.
    async fn load(&self, folder: PathBuf) -> Result<Option<Resolved<Arc<Placed>>>, Error> {
        let path = folder.join(MANIFEST);
        let Some(stamp) = Self::stamped(&path).await? else { return Ok(None) };
        if let Some(cached) =
            self.cache.lock().unwrap_or_else(PoisonError::into_inner).get(&folder).filter(|c| c.stamp == stamp)
        {
            return Ok(Some(Resolved { deploy: Arc::clone(&cached.deploy), scope: Arc::clone(&cached.placed) }));
        }
        let bytes = Self::slurp(&path).await?;
        let written: Unchecked =
            serde_json::from_slice(&bytes).map_err(|source| Error::Manifest { path: path.clone(), source })?;
        let checked = written.check(&path)?;
        let files: BTreeMap<Hash, ContentPath> = checked.files.into_iter().rev().collect();
        if let Some(entry) = checked.manifest.entries().iter().find(|entry| !files.contains_key(&entry.blob())) {
            return Err(Error::Unplaced { route: entry.route().clone() });
        }
        logging::debug!(
            name: logging::event::FOLDER_LOADED,
            folder = logging::Located(&folder).field(),
            bytes = logging::Size::from(bytes.len()).field(),
            files = logging::Count::from(files.len()).field(),
        );
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
            Some(folder) => {
                let loaded = async {
                    self.unlinked(&folder).await?;
                    self.load(folder).await
                };
                loaded.await.inspect_err(Error::said)
            }
            None => Ok(None),
        }
    }
}
