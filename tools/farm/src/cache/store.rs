//! The passes: what ran and passed, found again by its key.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::hash::Hash;
use crate::cache::hash::key::Key;
use crate::cache::{Ago, Fault, Moment};
use crate::config::jobs::Limits;

/// The JSON files of the cache.
#[derive(Debug, Clone, Copy)]
pub(super) struct Files;

impl Files {
    /// Reads one; none when it is not there.
    pub(super) fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, Fault> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                serde_json::from_str(&text).map(Some).map_err(|source| Fault::Json { path: path.to_path_buf(), source })
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Fault::Io { path: path.to_path_buf(), source }),
        }
    }
}

/// A pass that was remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) job: String,
    pub(crate) command: String,
    /// The id of the run that passed.
    pub(crate) id: String,
    pub(crate) host: String,
    pub(crate) workspace: String,
    pub(crate) at: Moment,
    /// Seconds the command ran.
    pub(crate) ran: u64,
    /// Its log, where the run that passed left it.
    pub(crate) log: PathBuf,
    pub(crate) digest: Hash,
}

/// A pass found, and how long ago it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hit {
    pub(crate) entry: Entry,
    pub(crate) ago: Ago,
}

/// The passes.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    dir: PathBuf,
    limits: Limits,
}

impl Store {
    pub(crate) fn new(dir: &Path, limits: Limits) -> Self {
        Self { dir: dir.join("results"), limits }
    }

    fn path(&self, key: Key) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// The pass under `key`, unless there is none or it is older than the limit.
    pub(crate) fn lookup(&self, key: Key, now: Moment) -> Result<Option<Hit>, Fault> {
        let path = self.path(key);
        let Some(entry) = Files::read::<Entry>(&path)? else {
            return Ok(None);
        };
        let age = entry.at.age(now);
        if age > self.limits.age() {
            std::fs::remove_file(&path).map_err(|source| Fault::Io { path, source })?;
            return Ok(None);
        }
        Ok(Some(Hit { entry, ago: Ago(age) }))
    }

    /// Remembers a pass, then forgets the expired ones and the oldest beyond the limit.
    pub(crate) fn record(&self, key: Key, entry: &Entry, now: Moment) -> Result<(), Fault> {
        let path = self.path(key);
        let io = |source| Fault::Io { path: path.clone(), source };
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let text = serde_json::to_string(entry).map_err(|source| Fault::Json { path: path.clone(), source })?;
        let temp = self.dir.join(format!("{key}.{}.tmp", std::process::id()));
        std::fs::write(&temp, text).map_err(io)?;
        std::fs::rename(&temp, &path).map_err(io)?;
        self.prune(now)
    }

    fn prune(&self, now: Moment) -> Result<(), Fault> {
        let listing = std::fs::read_dir(&self.dir).map_err(|source| Fault::Io { path: self.dir.clone(), source })?;
        let mut kept = Vec::new();
        for item in listing {
            let path = item.map_err(|source| Fault::Io { path: self.dir.clone(), source })?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            match Files::read::<Entry>(&path) {
                Ok(Some(entry)) if entry.at.age(now) <= self.limits.age() => kept.push((entry.at, path)),
                Ok(_) | Err(Fault::Json { .. }) => {
                    std::fs::remove_file(&path).map_err(|source| Fault::Io { path, source })?;
                }
                Err(other) => return Err(other),
            }
        }
        kept.sort();
        let extra = kept.len().saturating_sub(self.limits.entries.get() as usize);
        for (_, path) in kept.into_iter().take(extra) {
            std::fs::remove_file(&path).map_err(|source| Fault::Io { path, source })?;
        }
        Ok(())
    }
}

/// Words nextest prints at the start of a line for a test that did not pass cleanly the first time.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Flaky;

impl Flaky {
    const MARKS: [&'static str; 4] = ["FLAKY", "RETRY", "TRY ", "TIMEOUT"];

    /// Whether a log that ended in a pass shows a retry or a timeout: such a pass is luck, and a
    /// stored luck would be served as a pass forever.
    pub(crate) fn shows(log: &str) -> bool {
        log.lines().any(|line| Self::MARKS.iter().any(|mark| line.trim_start().starts_with(mark)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::tests::{Dir, entry, facts};

    #[test]
    fn a_pass_is_found_by_its_key_and_forgotten_when_old_or_beyond_the_count() {
        let dir = Dir::new("store");
        let limits = Limits {
            hours: std::num::NonZeroU64::MIN.saturating_add(1),
            entries: std::num::NonZeroU32::MIN.saturating_add(1),
        };
        let store = Store::new(&dir.0, limits);
        let words = vec!["x".to_owned()];
        let key = |n| Key::of(&facts(&words, n));
        assert_eq!(store.lookup(key(1), Moment(1000)).unwrap(), None, "a miss");
        store.record(key(1), &entry(1000), Moment(1000)).unwrap();
        let hit = store.lookup(key(1), Moment(1000 + 150)).unwrap().unwrap();
        assert_eq!((hit.entry.id.as_str(), hit.ago.to_string()), ("petal.1a", "2 min".to_owned()));
        assert_eq!(store.lookup(key(2), Moment(1100)).unwrap(), None, "another key is a miss");
        assert_eq!(store.lookup(key(1), Moment(1000 + 2 * 3600 + 1)).unwrap(), None, "past the age");
        assert!(!store.path(key(1)).exists(), "an expired pass is removed");
        for (n, at) in [(1, 100), (2, 200), (3, 300)] {
            store.record(key(n), &entry(at), Moment(300)).unwrap();
        }
        assert_eq!(store.lookup(key(1), Moment(300)).unwrap(), None, "the oldest went past the count of two");
        assert!(
            store.lookup(key(2), Moment(300)).unwrap().is_some()
                && store.lookup(key(3), Moment(300)).unwrap().is_some()
        );
    }

    #[test]
    fn a_retry_or_a_timeout_in_a_log_is_luck() {
        assert!(Flaky::shows("        PASS [ 0.1s] a b\n        FLAKY 2/3 [ 0.5s] a c\n"));
        assert!(Flaky::shows("       TRY 2 PASS [ 0.1s] a c"));
        assert!(Flaky::shows("     TIMEOUT [ 5.0s] a d"));
        assert!(!Flaky::shows("        PASS [ 0.1s] a b\n     Summary 1 test run: 1 passed"));
    }
}
