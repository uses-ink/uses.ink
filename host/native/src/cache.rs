//! An [`engine::Cache`] in a folder on disk, shared by every build and every process that opens
//! it.
//!
//! The folder holds [`Disk::LABEL`], so no read takes it as content wherever it is; `value/`, a
//! [`Directory`] of values by their hash, so a value two entries share is kept once and a value
//! that no longer hashes to its name is found out; and `key/<kind>/<digest>`, one small entry
//! per key listing its values' hashes and ending with the hash of that list. Every file is
//! written under a name of its own and renamed into place, so a reader in another process sees
//! the whole file or none of it. An entry that does not check out (its list, or a value it
//! names) is dropped and said, and the work is made again.
//!
//! The cache is bounded: [`Disk::sweep`] drops the entries used longest ago, by their files'
//! modification time, which a hit moves to now, until the values fit [`Disk::bound`].

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use engine::cache::{Cache, Key};
use model::{Announcement, Bytes, Hash, Notice};

use crate::blob::{Directory, Partial, StoreError};

/// A cache in a folder.
#[derive(Debug)]
pub struct Disk {
    root: PathBuf,
    values: Directory,
    /// The most bytes of values [`Disk::sweep`] leaves.
    pub bound: Bytes,
    /// What it has to say, until [`Disk::notices`] takes it.
    said: Mutex<Vec<Notice>>,
    /// Whether the folder is made, once the first value is put; `false` when it cannot be.
    made: OnceLock<bool>,
}

/// One entry, as [`Disk::sweep`] weighs it.
#[derive(Debug)]
struct Weighed {
    path: PathBuf,
    used: SystemTime,
    values: Vec<Hash>,
}

impl Disk {
    /// The file that marks a folder as a cache.
    pub const LABEL: &'static str = ".ink-cache";

    /// What [`Disk::LABEL`] holds.
    pub const MARK: &'static str = "uses.cache/v1\n";

    /// How an entry starts.
    const ENTRY: &'static str = "uses.cache/entry/v1\n";

    /// The bound when none is set: room for the copies of some thousand photos.
    pub const BOUND: Bytes = Bytes(2 << 30);

    /// The cache in `root`. Nothing is written until the first value is put, so a site with
    /// nothing to keep leaves no folder; a folder made then says `render.cache.cold`.
    pub fn open(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let values = Directory::at(root.join("value"));
        Self { root, values, bound: Self::BOUND, said: Mutex::default(), made: OnceLock::new() }
    }

    /// Its folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Makes the folder and its label the first time it is called; whether the folder holds a
    /// cache now. A folder that cannot be made is said once, as `cache.folder.unusable`.
    fn ready(&self) -> bool {
        *self.made.get_or_init(|| {
            let label = self.root.join(Self::LABEL);
            if label.is_file() {
                return true;
            }
            let made = std::fs::create_dir_all(self.values.root())
                .map_err(|source| StoreError::Create { path: self.values.root().to_path_buf(), source })
                .and_then(|()| Partial::new(&self.root, Self::LABEL).write(Self::MARK.as_bytes(), &label));
            let folder = self.root.display().to_string();
            match made {
                Ok(()) => self.say(Announcement::RenderCacheCold { folder }),
                Err(_) => self.say(Announcement::CacheFolderUnusable { folder }),
            }
            made.is_ok()
        })
    }

    /// Everything it has said since it was opened or last asked, in the order it said it.
    pub fn notices(&self) -> Vec<Notice> {
        std::mem::take(&mut *self.said.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Drops the entries used longest ago until the values fit [`Disk::bound`], then the values
    /// no entry names any more; said as `cache.entry.evicted`. Within the bound it removes
    /// nothing, so it never races a build putting values before their entry.
    pub fn sweep(&self) {
        let mut sizes: BTreeMap<Hash, u64> = BTreeMap::new();
        for (name, size) in Listing(self.values.root()).files() {
            if let Ok(hash) = name.parse::<Hash>() {
                sizes.insert(hash, size);
            }
        }
        let total: u64 = sizes.values().sum();
        if total <= self.bound.count() {
            return;
        }
        let evicted = self.evict(&sizes);
        if evicted > 0 {
            self.say(Announcement::CacheEntryEvicted { count: evicted, limit: self.bound });
        }
    }

    /// Removes entries, used longest ago first, until the values the rest name fit the
    /// bound, then every value no entry names; how many entries it removed.
    fn evict(&self, sizes: &BTreeMap<Hash, u64>) -> u64 {
        let mut entries = self.weigh();
        // used longest ago first; the path breaks ties, so two sweeps agree
        entries.sort_by(|a, b| (a.used, &a.path).cmp(&(b.used, &b.path)));
        let mut named: BTreeMap<Hash, usize> = BTreeMap::new();
        for entry in &entries {
            for hash in &entry.values {
                *named.entry(*hash).or_default() += 1;
            }
        }
        let mut kept: u64 = named.keys().filter_map(|hash| sizes.get(hash)).sum();
        let mut evicted = 0u64;
        for entry in &entries {
            if kept <= self.bound.count() {
                break;
            }
            if std::fs::remove_file(&entry.path).is_err() {
                continue;
            }
            evicted += 1;
            for hash in &entry.values {
                if let Some(count) = named.get_mut(hash) {
                    *count -= 1;
                    if *count == 0 {
                        kept = kept.saturating_sub(sizes.get(hash).copied().unwrap_or(0));
                    }
                }
            }
        }
        for hash in sizes.keys().filter(|hash| named.get(hash).is_none_or(|&count| count == 0)) {
            if let Err(_stays) = std::fs::remove_file(self.values.path(hash)) {
                // it stays, and the next sweep weighs it again
            }
        }
        evicted
    }

    /// Every entry, with when it was last used and what it names; an entry that cannot be read
    /// names nothing and goes first.
    fn weigh(&self) -> Vec<Weighed> {
        let mut entries = Vec::new();
        for kind in Listing(&self.root.join("key")).folders() {
            for (name, _) in Listing(&kind).files() {
                let path = kind.join(name);
                let used = std::fs::metadata(&path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
                let values = std::fs::read(&path).ok().and_then(|bytes| Entry::parse(&bytes)).unwrap_or_default();
                entries.push(Weighed { path, used, values });
            }
        }
        entries
    }

    /// The file of `key`'s entry.
    fn entry(&self, key: &Key) -> PathBuf {
        self.root.join("key").join(key.kind.name()).join(key.digest.to_string())
    }

    fn say(&self, announcement: Announcement) {
        self.said.lock().unwrap_or_else(PoisonError::into_inner).push(Notice::new(announcement));
    }

    /// Drops `key`'s entry and says it was corrupt.
    fn corrupt(&self, key: &Key) {
        match std::fs::remove_file(self.entry(key)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                self.say(Announcement::CacheEntryUnwritten { entry: key.to_string() });
            }
            _removed => {}
        }
        self.say(Announcement::CacheEntryCorrupt { entry: key.to_string() });
    }

    /// Writes `values` and then the entry naming them.
    fn write(&self, key: &Key, values: &[&[u8]]) -> Result<(), StoreError> {
        let mut hashes = Vec::with_capacity(values.len());
        for bytes in values {
            hashes.push(self.values.write(bytes)?);
        }
        let path = self.entry(key);
        let folder = path.parent().unwrap_or(&self.root);
        std::fs::create_dir_all(folder).map_err(|source| StoreError::Create { path: folder.to_path_buf(), source })?;
        let name = key.digest.to_string();
        Partial::new(folder, &name).write(&Entry::write(&hashes), &path)
    }
}

impl Cache for Disk {
    fn get(&self, key: &Key) -> Option<Vec<Vec<u8>>> {
        let path = self.entry(key);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(_) => {
                self.corrupt(key);
                return None;
            }
        };
        let Some(hashes) = Entry::parse(&bytes) else {
            self.corrupt(key);
            return None;
        };
        let mut values = Vec::with_capacity(hashes.len());
        for hash in &hashes {
            let Ok(bytes) = engine::Blobs::read(&self.values, hash) else {
                // a value that does not hash to its name would be found again by every write
                // of the right bytes, which sees the name taken
                if let Err(error) = std::fs::remove_file(self.values.path(hash))
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    self.say(Announcement::CacheEntryUnwritten { entry: key.to_string() });
                }
                self.corrupt(key);
                return None;
            };
            values.push(bytes.into_owned());
        }
        // a hit is a use: the entry is the last the sweep drops
        let touched = File::options().write(true).open(&path).and_then(|file| file.set_modified(SystemTime::now()));
        if touched.is_err() {
            self.say(Announcement::CacheEntryUnwritten { entry: key.to_string() });
        }
        Some(values)
    }

    fn put(&self, key: &Key, values: &[&[u8]]) {
        if !self.ready() {
            return;
        }
        if self.write(key, values).is_err() {
            self.say(Announcement::CacheEntryUnwritten { entry: key.to_string() });
        }
    }

    fn reject(&self, key: &Key) {
        self.corrupt(key);
    }
}

/// An entry's text: [`Disk::ENTRY`], one value's hash a line, then the hash of all that.
#[derive(Debug)]
struct Entry;

impl Entry {
    fn write(hashes: &[Hash]) -> Vec<u8> {
        let mut text = String::from(Disk::ENTRY);
        for hash in hashes {
            text.push_str(&hash.to_string());
            text.push('\n');
        }
        let check = Hash::of(text.as_bytes());
        text.push_str(&check.to_string());
        text.push('\n');
        text.into_bytes()
    }

    /// The hashes an entry names, or `None` when it does not check out.
    fn parse(bytes: &[u8]) -> Option<Vec<Hash>> {
        let text = std::str::from_utf8(bytes).ok()?;
        let body = text.strip_suffix('\n')?;
        let (listed, check) = body.rsplit_once('\n')?;
        let listed = text.get(..=listed.len())?;
        if check.parse::<Hash>().ok()? != Hash::of(listed.as_bytes()) {
            return None;
        }
        let lines = listed.strip_prefix(Disk::ENTRY)?;
        lines.lines().map(|line| line.parse().ok()).collect()
    }
}

/// A folder's children, by kind; a folder that cannot be listed has none.
#[derive(Clone, Copy, Debug)]
struct Listing<'a>(&'a Path);

impl Listing<'_> {
    /// Its files' names and sizes, in byte order of the names.
    fn files(self) -> BTreeSet<(String, u64)> {
        let Ok(children) = std::fs::read_dir(self.0) else { return BTreeSet::new() };
        children
            .filter_map(Result::ok)
            .filter_map(|child| {
                let metadata = child.metadata().ok().filter(std::fs::Metadata::is_file)?;
                Some((child.file_name().into_string().ok()?, metadata.len()))
            })
            .collect()
    }

    /// Its folders, in byte order.
    fn folders(self) -> BTreeSet<PathBuf> {
        let Ok(children) = std::fs::read_dir(self.0) else { return BTreeSet::new() };
        children.filter_map(Result::ok).map(|child| child.path()).filter(|path| path.is_dir()).collect()
    }
}
