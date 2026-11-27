//! What a job reads, as a digest: the content of the tracked files of the workspace that the
//! job's `inputs` select, plus the files every job reads (`Cargo.lock`, `rust-toolchain.toml`,
//! `.config/**`). The rsync of the sync compares by checksum but exposes no digest, so this
//! lists the files the way the sync sees them (jj's tracked files, minus what the sync leaves
//! out) and hashes their bytes. A cache of file hashes under `target/farm/`, keyed by (path,
//! size, mtime, inode), makes a second digest of an unchanged tree a stat per file.

pub(crate) mod glob;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cache::hash::{Hash, Hasher};
use crate::cache::inputs::glob::Selection;
use crate::cli::name::Excluded;
use crate::host::transport::process::{Failure, Process};

/// What the sync never ships and a digest never reads: build output and anything that looks like
/// a secret. The repo's own list (`[sync] exclude`) comes on top.
pub(crate) const BUILT_IN: [&str; 5] = ["target", ".env", ".env.*", "*.pem", "*.key"];

/// How the files of a workspace are listed.
pub(crate) trait Tracked: fmt::Debug + Send + Sync {
    /// The files below `root`, relative, with `/` between segments.
    fn files(&self, root: &Path) -> Result<Vec<String>, Fault>;
}

/// jj's list: every file the working copy tracks, which is what a sync would ship. jj snapshots
/// the working copy first, so a file made a moment ago is in it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Jj;

impl Tracked for Jj {
    fn files(&self, root: &Path) -> Result<Vec<String>, Fault> {
        let mut jj = Command::new("jj");
        jj.arg("-R").arg(root).args(["file", "list", "--no-pager", "-T", "path ++ \"\\n\""]).current_dir(root);
        let text = Process::from(jj).text(&[]).map_err(Fault::List)?;
        Ok(text.lines().map(str::to_owned).collect())
    }
}

/// Every file below the root except `.jj` and the top `target`: for tests, which run in
/// directories that are no jj workspace.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Walk;

#[cfg(test)]
impl Walk {
    fn below(dir: &Path, root: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            if matches!(relative.as_str(), ".jj" | "target") {
                continue;
            }
            if path.is_dir() {
                Self::below(&path, root, found);
            } else {
                found.push(relative);
            }
        }
    }
}

#[cfg(test)]
impl Tracked for Walk {
    fn files(&self, root: &Path) -> Result<Vec<String>, Fault> {
        let mut found = Vec::new();
        Self::below(root, root, &mut found);
        Ok(found)
    }
}

/// A digest that could not be made.
#[derive(Debug)]
pub(crate) enum Fault {
    /// The listing of tracked files failed.
    List(Failure),
    /// A listed file cannot be read.
    Read { path: PathBuf, source: std::io::Error },
    /// The file-hash cache cannot be written.
    Save { path: PathBuf, source: std::io::Error },
    /// An exclude that is not a glob.
    Skip(glob::Fault),
    /// `rust-toolchain.toml` is not TOML.
    Toolchain { path: PathBuf, source: Box<toml::de::Error> },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::List(_) => write!(out, "`jj file list` failed, so the tracked files are not known"),
            Self::Read { path, .. } => write!(out, "cannot read {}", path.display()),
            Self::Save { path, .. } => write!(out, "cannot write the file-hash cache {}", path.display()),
            Self::Skip(_) => write!(out, "an excluded name is not a glob"),
            Self::Toolchain { path, .. } => write!(out, "{} is not TOML", path.display()),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::List(source) => Some(source),
            Self::Read { source, .. } | Self::Save { source, .. } => Some(source),
            Self::Skip(source) => Some(source),
            Self::Toolchain { source, .. } => Some(source.as_ref()),
        }
    }
}

/// What a stat said about a file: enough to know it did not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stat {
    size: u64,
    mtime: u128,
    inode: u64,
}

impl Stat {
    fn of(meta: &std::fs::Metadata) -> Self {
        let mtime =
            meta.modified().ok().and_then(|at| at.duration_since(UNIX_EPOCH).ok()).map_or(0, |since| since.as_nanos());
        Self { size: meta.len(), mtime, inode: meta.ino() }
    }

    /// Whether the file was written so lately that a later write could keep this stat.
    fn racy(&self, now: SystemTime) -> bool {
        let written = UNIX_EPOCH + Duration::from_nanos(u64::try_from(self.mtime).unwrap_or(u64::MAX));
        now.duration_since(written).is_ok_and(|age| age < Self::SETTLE)
    }

    const SETTLE: Duration = Duration::from_secs(2);
}

/// The hashes of files by path, as the last digest left them in `target/farm/hashes.tsv`.
#[derive(Debug, Default)]
struct Hashes {
    known: HashMap<String, (Stat, Hash)>,
    dirty: bool,
}

impl Hashes {
    /// Where it lives, below the workspace.
    fn file(root: &Path) -> PathBuf {
        root.join("target").join("farm").join("hashes.tsv")
    }

    /// Reads the file; a missing or damaged one is an empty cache, which only costs time.
    fn read(root: &Path) -> Self {
        let text = std::fs::read_to_string(Self::file(root)).unwrap_or_default();
        let known = text.lines().filter_map(Self::line).collect();
        Self { known, dirty: false }
    }

    fn line(line: &str) -> Option<(String, (Stat, Hash))> {
        let mut parts = line.split('\t');
        let path = parts.next()?.to_owned();
        let size = parts.next()?.parse().ok()?;
        let mtime = parts.next()?.parse().ok()?;
        let inode = parts.next()?.parse().ok()?;
        let hash = parts.next()?.parse().ok()?;
        Some((path, (Stat { size, mtime, inode }, hash)))
    }

    fn get(&self, path: &str, stat: Stat) -> Option<Hash> {
        self.known.get(path).filter(|(seen, _)| *seen == stat).map(|(_, hash)| *hash)
    }

    fn put(&mut self, path: &str, stat: Stat, hash: Hash) {
        if !path.contains(['\t', '\n']) && self.known.get(path) != Some(&(stat, hash)) {
            self.known.insert(path.to_owned(), (stat, hash));
            self.dirty = true;
        }
    }

    /// Writes it back when it changed, keeping only the paths of `live`.
    fn save(mut self, root: &Path, live: &[&str]) -> Result<(), Fault> {
        let before = self.known.len();
        self.known.retain(|path, _| live.binary_search(&path.as_str()).is_ok());
        if !self.dirty && self.known.len() == before {
            return Ok(());
        }
        let path = Self::file(root);
        let mut text = String::new();
        let mut rows: Vec<_> = self.known.iter().collect();
        rows.sort_by_key(|(path, _)| path.as_str());
        for (name, (stat, hash)) in rows {
            writeln!(text, "{name}\t{}\t{}\t{}\t{hash}", stat.size, stat.mtime, stat.inode).ok();
        }
        let save = |source| Fault::Save { path: path.clone(), source };
        let dir = path.parent().unwrap_or(root);
        std::fs::create_dir_all(dir).map_err(save)?;
        let temp = dir.join(format!("hashes.{}.tmp", std::process::id()));
        std::fs::write(&temp, text).map_err(save)?;
        std::fs::rename(&temp, &path).map_err(save)
    }
}

/// The tracked files of a workspace and the hash of each, sorted by path.
#[derive(Debug)]
struct Table(Vec<(String, Hash)>);

/// The digest maker of one run: it lists and hashes the tree once and answers every job from it.
#[derive(Debug)]
pub(crate) struct Inputs {
    tracked: Arc<dyn Tracked>,
    skip: Selection,
    memo: Mutex<Option<Arc<Table>>>,
}

impl Inputs {
    /// The files every job reads, whatever its `inputs` say.
    fn forced(path: &str) -> bool {
        path == "Cargo.lock" || path == "rust-toolchain.toml" || path.starts_with(".config/")
    }

    /// A maker that leaves out the built-in names and `exclude`.
    pub(crate) fn new(tracked: Arc<dyn Tracked>, exclude: &[Excluded]) -> Result<Self, Fault> {
        let mut globs = vec!["**".to_owned()];
        globs.extend(BUILT_IN.iter().map(|name| format!("!{name}")));
        globs.extend(exclude.iter().map(|name| format!("!{name}")));
        let skip = Selection::parse(&globs).map_err(|(_, fault)| Fault::Skip(fault))?;
        Ok(Self { tracked, skip, memo: Mutex::new(None) })
    }

    /// The digest of the files `files` selects (every file when none), with the forced ones.
    /// `again` hashes the tree anew instead of reusing what an earlier call of this run found.
    pub(crate) fn digest(&self, root: &Path, files: Option<&Selection>, again: bool) -> Result<Hash, Fault> {
        let table = self.table(root, again)?;
        let mut hasher = Hasher::default();
        for (path, hash) in &table.0 {
            if Self::forced(path) || files.is_none_or(|files| files.selects(path)) {
                hasher.field(path.as_bytes()).field(hash.to_string().as_bytes());
            }
        }
        Ok(hasher.finish())
    }

    fn table(&self, root: &Path, again: bool) -> Result<Arc<Table>, Fault> {
        let mut memo = self.memo.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(table) = memo.as_ref().filter(|_| !again) {
            return Ok(Arc::clone(table));
        }
        let mut listed = self.tracked.files(root)?;
        listed.sort();
        listed.dedup();
        listed.retain(|path| self.skip.selects(path));
        let mut hashes = Hashes::read(root);
        let now = SystemTime::now();
        let mut rows = Vec::with_capacity(listed.len());
        for path in &listed {
            let full = root.join(path);
            let read = |source| Fault::Read { path: full.clone(), source };
            let meta = std::fs::symlink_metadata(&full).map_err(read)?;
            if meta.is_dir() {
                continue;
            }
            let stat = Stat::of(&meta);
            let hash = if let Some(hash) = hashes.get(path, stat) {
                hash
            } else {
                let hash = Self::content(&full, &meta).map_err(read)?;
                if !stat.racy(now) {
                    hashes.put(path, stat, hash);
                }
                hash
            };
            rows.push((path.clone(), hash));
        }
        let live: Vec<&str> = listed.iter().map(String::as_str).collect();
        hashes.save(root, &live)?;
        let table = Arc::new(Table(rows));
        *memo = Some(Arc::clone(&table));
        Ok(table)
    }

    /// The hash of a file's bytes, or of where a symlink points.
    fn content(path: &Path, meta: &std::fs::Metadata) -> std::io::Result<Hash> {
        let mut hasher = Hasher::default();
        if meta.file_type().is_symlink() {
            hasher.field(std::fs::read_link(path)?.as_os_str().as_encoded_bytes());
            return Ok(hasher.finish());
        }
        let mut file = std::fs::File::open(path)?;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                return Ok(hasher.finish());
            }
            hasher.bytes(buffer.get(..read).unwrap_or_default());
        }
    }
}

/// The toolchain channel the repo pins, from `rust-toolchain.toml`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Toolchain;

impl Toolchain {
    /// The channel, or none when there is no file or it names none.
    pub(crate) fn channel(root: &Path) -> Result<Option<String>, Fault> {
        let path = root.join("rust-toolchain.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Fault::Read { path, source }),
        };
        let table: toml::Table =
            toml::from_str(&text).map_err(|source| Fault::Toolchain { path, source: Box::new(source) })?;
        Ok(table
            .get("toolchain")
            .and_then(|toolchain| toolchain.get("channel"))
            .and_then(toml::Value::as_str)
            .map(str::to_owned))
    }
}
