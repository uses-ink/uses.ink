//! What the farm remembers on this machine so a job is not run twice for nothing, kept in one
//! directory per repo (`$XDG_CACHE_HOME/uses-ink/farm/<repo>`, else `~/.cache/...`), shared by
//! every workspace of the repo:
//!
//! - [`Store`]: the passes. A job that passed is keyed by [`Key`] (its name, command, input
//!   digest, host class, toolchain and job file), so a rebase onto a tree with the same inputs
//!   finds it. Failures are never stored.
//! - [`Marks`]: the jobs running now, so an identical job attaches to the one running.
//! - [`Journal`]: what each workspace ran, for `farm status` and the budget.

pub(crate) mod hash;
pub(crate) mod inputs;

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cache::hash::{Hash, Hasher};
use crate::cache::inputs::Inputs;
use crate::config::jobs::Limits;
use crate::place::history::{History, Memory};
use crate::place::wake::Seen;

/// A time as whole seconds since the epoch: what the cache files hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Moment(pub(crate) u64);

impl Moment {
    /// A moment of the system clock.
    pub(crate) fn now() -> Self {
        Self::of(SystemTime::now())
    }

    /// A moment of the system clock.
    pub(crate) fn of(time: SystemTime) -> Self {
        Self(time.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()))
    }

    /// How long before `now` this was.
    pub(crate) const fn age(self, now: Self) -> Duration {
        Duration::from_secs(now.0.saturating_sub(self.0))
    }
}

/// A span as a person says it: `40 s`, `3 min`, `5 h`, `2 d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ago(pub(crate) Duration);

impl fmt::Display for Ago {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        match secs {
            0..=59 => write!(out, "{secs} s"),
            60..=3599 => write!(out, "{} min", secs / 60),
            3600..=86_399 => write!(out, "{} h", secs / 3600),
            _ => write!(out, "{} d", secs / 86_400),
        }
    }
}

/// What a cache call cannot do.
#[derive(Debug)]
pub(crate) enum Fault {
    Io { path: PathBuf, source: std::io::Error },
    Json { path: PathBuf, source: serde_json::Error },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } => write!(out, "cannot use the cache file {}", path.display()),
            Self::Json { path, .. } => write!(out, "the cache file {} is not what the farm wrote", path.display()),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
        }
    }
}

/// Why the cache was not used for a job: said once, and the job runs as if there were none.
#[derive(Debug)]
pub(crate) enum Trouble {
    Inputs(crate::cache::inputs::Fault),
    Store(Fault),
    /// Waiting for an identical job failed.
    Attach(crate::cli::error::Error),
    /// There is no cache.
    Off,
}

impl From<crate::cache::inputs::Fault> for Trouble {
    fn from(fault: crate::cache::inputs::Fault) -> Self {
        Self::Inputs(fault)
    }
}

impl From<Fault> for Trouble {
    fn from(fault: Fault) -> Self {
        Self::Store(fault)
    }
}

impl fmt::Display for Trouble {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inputs(_) => write!(out, "the inputs of the job could not be digested"),
            Self::Store(_) => write!(out, "the cache could not be used"),
            Self::Attach(_) => write!(out, "attaching to the identical job failed"),
            Self::Off => write!(out, "there is no cache directory"),
        }
    }
}

impl std::error::Error for Trouble {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inputs(source) => Some(source),
            Self::Store(source) => Some(source),
            Self::Attach(source) => Some(source),
            Self::Off => None,
        }
    }
}

/// An error and its causes on one line: `a: caused by b: caused by c`.
#[derive(Debug)]
pub(crate) struct Causes<'a>(pub(crate) &'a (dyn std::error::Error + 'static));

impl fmt::Display for Causes<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(error) = cause {
            write!(out, "; caused by: {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}

/// Everything a pass is keyed by.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Facts<'a> {
    pub(crate) job: &'a str,
    pub(crate) words: &'a [String],
    /// The job's own environment variables.
    pub(crate) env: &'a [String],
    pub(crate) digest: Hash,
    /// The architecture of the host, for a job whose output depends on it.
    pub(crate) arch: Option<&'a str>,
    /// The channel `rust-toolchain.toml` pins.
    pub(crate) channel: Option<&'a str>,
    /// The digest of the job file.
    pub(crate) config: Hash,
}

/// The key of a pass: the hash of its [`Facts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key(Hash);

impl Key {
    pub(crate) fn of(facts: &Facts<'_>) -> Self {
        let mut hasher = Hasher::default();
        hasher.field(b"farm-pass-1").field(facts.job.as_bytes());
        for word in facts.words {
            hasher.field(word.as_bytes());
        }
        hasher.field(b"env");
        for var in facts.env {
            hasher.field(var.as_bytes());
        }
        hasher.field(facts.digest.to_string().as_bytes());
        hasher.field(facts.arch.unwrap_or("any").as_bytes());
        hasher.field(facts.channel.unwrap_or("none").as_bytes());
        hasher.field(facts.config.to_string().as_bytes());
        Self(hasher.finish())
    }

    /// The hash, for a mark to carry.
    pub(crate) const fn hash(self) -> Hash {
        self.0
    }

    /// The key of the same job running in one workspace: what a second copy is told apart by.
    pub(crate) fn within(self, workspace: &str) -> Self {
        Self(
            Hasher::default()
                .field(b"farm-run-1")
                .field(self.0.to_string().as_bytes())
                .field(workspace.as_bytes())
                .finish(),
        )
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}

/// The JSON files of the cache.
#[derive(Debug, Clone, Copy)]
struct Files;

impl Files {
    /// Reads one; none when it is not there.
    fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, Fault> {
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

/// A job running now, which an identical job may attach to, and what a pass of it will be keyed by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Mark {
    pub(crate) id: String,
    /// The process that started it, which is still working for it while it is alive.
    pub(crate) pid: u32,
    pub(crate) key: Hash,
    pub(crate) job: String,
    pub(crate) command: String,
    pub(crate) workspace: String,
    pub(crate) host: String,
    pub(crate) digest: Hash,
}

/// How a claim on a running job ended.
#[derive(Debug)]
pub(crate) enum Claim {
    /// No identical job was running: this one is, until the guard drops.
    Won(Claimed),
    /// One is: attach to it.
    Held(Mark),
}

/// A claim on a running job, released when it drops unless it was told to stay: a detached job
/// is still running when its client leaves.
#[derive(Debug)]
pub(crate) struct Claimed {
    marks: Marks,
    key: Key,
    stay: bool,
}

impl Claimed {
    /// Leaves the mark in place: the job outlives the client.
    pub(crate) const fn stay(&mut self) {
        self.stay = true;
    }
}

impl Drop for Claimed {
    fn drop(&mut self) {
        if !self.stay {
            self.marks.release(self.key);
        }
    }
}

/// The jobs running now.
#[derive(Debug, Clone)]
pub(crate) struct Marks {
    dir: PathBuf,
}

impl Marks {
    pub(crate) fn new(dir: &Path) -> Self {
        Self { dir: dir.join("running") }
    }

    fn path(&self, key: Key) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// Marks the job as running, or says which identical job already is. The mark is made whole
    /// by a hard link, which fails if one exists, so two clients never both win.
    pub(crate) fn claim(&self, key: Key, mark: &Mark) -> Result<Claim, Fault> {
        let path = self.path(key);
        let io = |source| Fault::Io { path: path.clone(), source };
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let temp = self.dir.join(format!("{}.tmp", mark.id));
        let text = serde_json::to_string(mark).map_err(|source| Fault::Json { path: path.clone(), source })?;
        std::fs::write(&temp, text).map_err(io)?;
        for _ in 0..3 {
            match std::fs::hard_link(&temp, &path) {
                Ok(()) => {
                    std::fs::remove_file(&temp).map_err(io)?;
                    return Ok(Claim::Won(Claimed { marks: self.clone(), key, stay: false }));
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => match Files::read::<Mark>(&path) {
                    Ok(Some(held)) => {
                        std::fs::remove_file(&temp).map_err(io)?;
                        return Ok(Claim::Held(held));
                    }
                    // released since the link failed, or unreadable: take it away and try again
                    Ok(None) => {}
                    Err(Fault::Json { .. }) => self.release(key),
                    Err(other) => return Err(other),
                },
                Err(source) => return Err(io(source)),
            }
        }
        std::fs::remove_file(&temp).ok();
        Err(io(std::io::Error::other("the mark kept changing under the claim")))
    }

    /// Takes the mark away; a mark that is gone is fine.
    pub(crate) fn release(&self, key: Key) {
        std::fs::remove_file(self.path(key)).ok();
    }

    /// The running job with this id, and the key of its mark.
    pub(crate) fn holding(&self, id: &str) -> Result<Option<(Key, Mark)>, Fault> {
        let listing = match std::fs::read_dir(&self.dir) {
            Ok(listing) => listing,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Fault::Io { path: self.dir.clone(), source }),
        };
        for item in listing {
            let path = item.map_err(|source| Fault::Io { path: self.dir.clone(), source })?.path();
            let stem = path.file_stem().and_then(|stem| stem.to_str()).and_then(|stem| stem.parse::<Hash>().ok());
            let (true, Some(stem)) = (path.extension().is_some_and(|extension| extension == "json"), stem) else {
                continue;
            };
            if let Some(mark) = Files::read::<Mark>(&path)?.filter(|mark| mark.id == id) {
                return Ok(Some((Key(stem), mark)));
            }
        }
        Ok(None)
    }

    /// Whether the process that started a job still runs.
    pub(crate) fn alive(pid: u32) -> bool {
        Path::new("/proc").join(pid.to_string()).exists()
    }

    /// The key a mark was filed under, for a mark read back.
    pub(crate) const fn key(hash: Hash) -> Key {
        Key(hash)
    }
}

/// What a journal line records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// A full `farm check`; `count` is the stages it ran, `secs` their host time.
    Check,
    /// A job run on a host; `secs` is its time there.
    Stage,
    /// A job served from the passes.
    Hit,
    /// A job attached to an identical one already running.
    Attach,
}

/// One line of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) at: Moment,
    pub(crate) workspace: String,
    pub(crate) kind: Kind,
    pub(crate) job: String,
    pub(crate) secs: u64,
    pub(crate) count: u32,
}

/// What one workspace did in a day.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Tally {
    pub(crate) workspace: String,
    pub(crate) checks: u32,
    /// Full checks that ran at least one stage: the heavy ones.
    pub(crate) heavy: u32,
    pub(crate) stages: u32,
    pub(crate) hits: u32,
    pub(crate) attached: u32,
    pub(crate) secs: u64,
}

impl fmt::Display for Tally {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}: {} full checks, {} stage runs, {} cache hits, {} deduped, {} host seconds",
            self.workspace, self.checks, self.stages, self.hits, self.attached, self.secs
        )
    }
}

/// What each workspace ran.
#[derive(Debug, Clone)]
pub(crate) struct Journal {
    file: PathBuf,
}

impl Journal {
    /// A week is kept; `farm status` shows a day.
    const KEEP: Duration = Duration::from_hours(168);
    const DAY: Duration = Duration::from_hours(24);
    /// The size past which an append also drops what is older than a week.
    const LARGE: u64 = 256 * 1024;

    pub(crate) fn new(dir: &Path) -> Self {
        Self { file: dir.join("journal.jsonl") }
    }

    pub(crate) fn append(&self, record: &Record) -> Result<(), Fault> {
        use std::io::Write as _;
        let io = |source| Fault::Io { path: self.file.clone(), source };
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let line = serde_json::to_string(record).map_err(|source| Fault::Json { path: self.file.clone(), source })?;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.file).map_err(io)?;
        writeln!(file, "{line}").map_err(io)?;
        if file.metadata().map_err(io)?.len() > Self::LARGE {
            self.trim(record.at)?;
        }
        Ok(())
    }

    fn read(&self) -> Result<Vec<Record>, Fault> {
        let text = match std::fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(Fault::Io { path: self.file.clone(), source }),
        };
        // a line cut short by a crash is skipped: the journal is a count, not a record of truth
        Ok(text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect())
    }

    fn trim(&self, now: Moment) -> Result<(), Fault> {
        let mut text = String::new();
        for record in self.read()?.into_iter().filter(|record| record.at.age(now) <= Self::KEEP) {
            text.push_str(
                &serde_json::to_string(&record).map_err(|source| Fault::Json { path: self.file.clone(), source })?,
            );
            text.push('\n');
        }
        std::fs::write(&self.file, text).map_err(|source| Fault::Io { path: self.file.clone(), source })
    }

    /// The last day, one tally per workspace, by name.
    pub(crate) fn tally(&self, now: Moment) -> Result<Vec<Tally>, Fault> {
        let mut tallies: Vec<Tally> = Vec::new();
        for record in self.read()?.into_iter().filter(|record| record.at.age(now) <= Self::DAY) {
            let at = if let Some(at) = tallies.iter().position(|tally| tally.workspace == record.workspace) {
                at
            } else {
                tallies.push(Tally { workspace: record.workspace.clone(), ..Tally::default() });
                tallies.len() - 1
            };
            let Some(tally) = tallies.get_mut(at) else { continue };
            match record.kind {
                Kind::Check => {
                    tally.checks += 1;
                    tally.heavy += u32::from(record.count > 0);
                }
                Kind::Stage => {
                    tally.stages += 1;
                    tally.secs += record.secs;
                }
                Kind::Hit => tally.hits += 1,
                Kind::Attach => tally.attached += 1,
            }
        }
        tallies.sort_by(|one, other| one.workspace.cmp(&other.workspace));
        Ok(tallies)
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

/// The cache of one run: the passes, the running jobs, the journal and the digest maker.
#[derive(Debug)]
pub(crate) struct Cache {
    pub(crate) store: Store,
    pub(crate) marks: Marks,
    pub(crate) journal: Journal,
    pub(crate) history: History,
    pub(crate) seen: Seen,
    pub(crate) inputs: Inputs,
}

impl Cache {
    pub(crate) fn at(dir: &Path, limits: Limits, memory: Memory, inputs: Inputs) -> Self {
        Self {
            store: Store::new(dir, limits),
            marks: Marks::new(dir),
            journal: Journal::new(dir),
            history: History::at(dir, memory),
            seen: Seen::at(dir),
            inputs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("farm-cache-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            Self(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn facts(words: &[String], digest: u128) -> Facts<'_> {
        Facts {
            job: "test",
            words,
            env: &[],
            digest: format!("{digest:032x}").parse().unwrap(),
            arch: Some("x86_64"),
            channel: Some("1.100.0"),
            config: Hasher::default().finish(),
        }
    }

    fn entry(at: u64) -> Entry {
        Entry {
            job: "test".to_owned(),
            command: "cargo xtask check test".to_owned(),
            id: "petal.1a".to_owned(),
            host: "petal".to_owned(),
            workspace: "w".to_owned(),
            at: Moment(at),
            ran: 30,
            log: PathBuf::from("/x/log"),
            digest: Hasher::default().finish(),
        }
    }

    #[test]
    fn the_key_follows_each_fact() {
        let words = vec!["cargo".to_owned(), "xtask".to_owned()];
        let base = Key::of(&facts(&words, 1));
        assert_eq!(base, Key::of(&facts(&words, 1)));
        assert_ne!(base, Key::of(&facts(&words, 2)), "the digest");
        let other = vec!["cargo".to_owned(), "xtask2".to_owned()];
        assert_ne!(base, Key::of(&facts(&other, 1)), "the command");
        let mut changed = facts(&words, 1);
        changed.arch = Some("aarch64");
        assert_ne!(base, Key::of(&changed), "the host class");
        changed = facts(&words, 1);
        changed.channel = Some("1.101.0");
        assert_ne!(base, Key::of(&changed), "the toolchain");
        changed = facts(&words, 1);
        changed.config = format!("{:032x}", 9).parse().unwrap();
        assert_ne!(base, Key::of(&changed), "the job file");
        changed = facts(&words, 1);
        changed.job = "lint";
        assert_ne!(base, Key::of(&changed), "the job name");
        assert_ne!(base.within("a"), base.within("b"));
    }

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
    fn two_identical_jobs_do_not_both_claim_and_a_released_claim_can_be_taken() {
        let dir = Dir::new("marks");
        let marks = Marks::new(&dir.0);
        let words = vec!["x".to_owned()];
        let key = Key::of(&facts(&words, 1)).within("w");
        let mark = |id: &str| Mark {
            id: id.to_owned(),
            pid: std::process::id(),
            key: Hasher::default().finish(),
            job: "test".to_owned(),
            command: "c".to_owned(),
            workspace: "w".to_owned(),
            host: "petal".to_owned(),
            digest: Hasher::default().finish(),
        };
        let Claim::Won(mut first) = marks.claim(key, &mark("petal.1")).unwrap() else { panic!("the first claim wins") };
        let Claim::Held(held) = marks.claim(key, &mark("petal.2")).unwrap() else {
            panic!("the second finds the first")
        };
        assert_eq!(held.id, "petal.1");
        assert_eq!(marks.holding("petal.1").unwrap().map(|(found, _)| found), Some(key));
        first.stay();
        drop(first);
        assert!(matches!(marks.claim(key, &mark("petal.3")).unwrap(), Claim::Held(_)), "a detached job keeps its mark");
        marks.release(key);
        let Claim::Won(claimed) = marks.claim(key, &mark("petal.4")).unwrap() else { panic!("free again") };
        drop(claimed);
        assert!(marks.holding("petal.4").unwrap().is_none(), "a claim that drops releases");
    }

    #[test]
    fn the_journal_counts_a_day_per_workspace() {
        let dir = Dir::new("journal");
        let journal = Journal::new(&dir.0);
        let record = |workspace: &str, kind, at, secs, count| Record {
            at: Moment(at),
            workspace: workspace.to_owned(),
            kind,
            job: "j".to_owned(),
            secs,
            count,
        };
        let day = 86_400;
        let now = 10 * day;
        for line in [
            record("a", Kind::Check, now - 10, 0, 2),
            record("a", Kind::Check, now - 20, 0, 0),
            record("a", Kind::Stage, now - 10, 30, 0),
            record("a", Kind::Stage, now - 10, 12, 0),
            record("a", Kind::Hit, now - 5, 0, 0),
            record("b", Kind::Attach, now - 5, 0, 0),
            record("a", Kind::Stage, now - day - 1, 99, 0),
        ] {
            journal.append(&line).unwrap();
        }
        let tallies = journal.tally(Moment(now)).unwrap();
        assert_eq!(tallies.len(), 2);
        let a = tallies.first().unwrap();
        assert_eq!((a.checks, a.heavy, a.stages, a.hits, a.attached, a.secs), (2, 1, 2, 1, 0, 42));
        assert_eq!(a.to_string(), "a: 2 full checks, 2 stage runs, 1 cache hits, 0 deduped, 42 host seconds");
        assert_eq!(tallies.get(1).unwrap().attached, 1);
    }

    #[test]
    fn a_retry_or_a_timeout_in_a_log_is_luck() {
        assert!(Flaky::shows("        PASS [ 0.1s] a b\n        FLAKY 2/3 [ 0.5s] a c\n"));
        assert!(Flaky::shows("       TRY 2 PASS [ 0.1s] a c"));
        assert!(Flaky::shows("     TIMEOUT [ 5.0s] a d"));
        assert!(!Flaky::shows("        PASS [ 0.1s] a b\n     Summary 1 test run: 1 passed"));
    }

    #[test]
    fn spans_read_as_a_person_says_them() {
        let say = |secs| Ago(Duration::from_secs(secs)).to_string();
        assert_eq!(
            (say(5), say(180), say(7200), say(200_000)),
            ("5 s".to_owned(), "3 min".to_owned(), "2 h".to_owned(), "2 d".to_owned())
        );
    }
}
