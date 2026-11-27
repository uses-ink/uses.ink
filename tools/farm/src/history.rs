//! How long jobs took on each host, so placement can pick the host that will finish first.
//!
//! The farm appends one [`Sample`] per finished job to `history.jsonl` in the repo's cache
//! directory. The file is bounded by age and by count ([`Memory`], the `[history]` table of the
//! job file): [`History::load`] drops what is older or beyond the newest `samples` and rewrites
//! the file when it dropped anything. Two farms loading at the same moment may lose a sample
//! appended during the rewrite; a lost sample only makes a median a little less certain.

use std::fmt;
use std::io::Write as _;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cache::{Fault, Moment};
use crate::place::Need;

/// The clock history is aged by: the wall clock, or a fixed one in tests.
pub(crate) trait Clock {
    fn now(&self) -> Moment;
}

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Wall;

impl Clock for Wall {
    fn now(&self) -> Moment {
        Moment::now()
    }
}

/// How much history is kept: the `[history]` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Memory {
    pub(crate) hours: NonZeroU64,
    pub(crate) samples: NonZeroU32,
}

impl Memory {
    /// Two weeks.
    const HOURS: NonZeroU64 = NonZeroU64::MIN.saturating_add(335);
    const SAMPLES: NonZeroU32 = NonZeroU32::MIN.saturating_add(1999);

    const fn age(self) -> Duration {
        Duration::from_secs(self.hours.get() * 3600)
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self { hours: Self::HOURS, samples: Self::SAMPLES }
    }
}

/// How many packages a cargo command names: the size class of its work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Scope {
    /// No `-p`, or `--workspace`.
    Workspace,
    /// One `-p`.
    Package,
    /// Several.
    Packages,
}

/// What a job is, for timing: a declared stage, or a cargo subcommand and its scope. The whole
/// command line is not the key, so `clippy -p a` and `clippy -p b` share their history.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum Key {
    Stage { name: String },
    Cargo { verb: String, scope: Scope },
}

impl Key {
    pub(crate) fn stage(name: &str) -> Self {
        Self::Stage { name: name.to_owned() }
    }

    /// The key of `cargo <args>`; what follows a `--` belongs to the tool, not to cargo.
    pub(crate) fn cargo(args: &[String]) -> Self {
        let own = args.split(|arg| arg == "--").next().unwrap_or_default();
        let named = own.iter().filter(|arg| matches!(arg.as_str(), "-p" | "--package")).count();
        let scope = match named {
            _ if own.iter().any(|arg| arg == "--workspace" || arg == "--all") => Scope::Workspace,
            0 => Scope::Workspace,
            1 => Scope::Package,
            _ => Scope::Packages,
        };
        Self::Cargo { verb: Need::subcommand(own).unwrap_or("cargo").to_owned(), scope }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stage { name } => out.write_str(name),
            Self::Cargo { verb, scope: Scope::Workspace } => write!(out, "cargo {verb}"),
            Self::Cargo { verb, scope: Scope::Package } => write!(out, "cargo {verb} -p"),
            Self::Cargo { verb, scope: Scope::Packages } => write!(out, "cargo {verb} -p..."),
        }
    }
}

/// One finished job: a line of the history file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Sample {
    pub(crate) at: Moment,
    pub(crate) key: Key,
    pub(crate) host: String,
    /// Wall seconds on the host, from the first compile to the exit.
    pub(crate) secs: u64,
    /// Whether it passed: only passes are timed for placement, a failure stops early.
    pub(crate) passed: bool,
}

/// A typical time: the median of the latest passes, and how many there were.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Median {
    pub(crate) secs: u64,
    pub(crate) runs: u32,
}

/// The samples of the file, read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Table {
    samples: Vec<Sample>,
}

impl Table {
    /// Only the latest passes of a key on a host count, so a host that got faster is believed.
    const RECENT: usize = 15;

    pub(crate) const fn new(samples: Vec<Sample>) -> Self {
        Self { samples }
    }

    /// Whether `host` has any history at all.
    pub(crate) fn knows(&self, host: &str) -> bool {
        self.samples.iter().any(|sample| sample.host == host)
    }

    /// The median pass of `key` on `host`; none with no pass.
    pub(crate) fn median(&self, key: &Key, host: &str) -> Option<Median> {
        let mut passes: Vec<&Sample> =
            self.samples.iter().filter(|s| s.passed && s.host == host && &s.key == key).collect();
        passes.sort_by_key(|sample| sample.at);
        let latest = passes.split_off(passes.len().saturating_sub(Self::RECENT));
        let mut secs: Vec<u64> = latest.iter().map(|sample| sample.secs).collect();
        secs.sort_unstable();
        let runs = u32::try_from(secs.len()).ok().filter(|runs| *runs > 0)?;
        let mid = secs.len() / 2;
        let typical =
            if secs.len() % 2 == 1 { *secs.get(mid)? } else { (secs.get(mid.checked_sub(1)?)? + secs.get(mid)?) / 2 };
        Some(Median { secs: typical, runs })
    }

    /// Every key `host` has a pass of, with its median, in key order.
    pub(crate) fn medians(&self, host: &str) -> Vec<(Key, Median)> {
        let mut keys: Vec<&Key> = self.samples.iter().filter(|s| s.passed && s.host == host).map(|s| &s.key).collect();
        keys.sort();
        keys.dedup();
        keys.into_iter().filter_map(|key| self.median(key, host).map(|median| (key.clone(), median))).collect()
    }
}

/// The history file.
#[derive(Debug, Clone)]
pub(crate) struct History {
    file: PathBuf,
    memory: Memory,
}

impl History {
    pub(crate) fn at(dir: &Path, memory: Memory) -> Self {
        Self { file: dir.join("history.jsonl"), memory }
    }

    /// Adds a sample.
    pub(crate) fn append(&self, sample: &Sample) -> Result<(), Fault> {
        let io = |source| Fault::Io { path: self.file.clone(), source };
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let line = serde_json::to_string(sample).map_err(|source| Fault::Json { path: self.file.clone(), source })?;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.file).map_err(io)?;
        writeln!(file, "{line}").map_err(io)
    }

    /// The samples within the bounds; a file that holds more is rewritten with only those. A
    /// line cut short by a crash is skipped.
    pub(crate) fn load(&self, clock: &dyn Clock) -> Result<Table, Fault> {
        let text = match std::fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Table::default()),
            Err(source) => return Err(Fault::Io { path: self.file.clone(), source }),
        };
        let now = clock.now();
        let lines = text.lines().count();
        let mut kept: Vec<Sample> = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Sample>(line).ok())
            .filter(|sample| sample.at.age(now) <= self.memory.age())
            .collect();
        kept.drain(..kept.len().saturating_sub(self.memory.samples.get() as usize));
        if kept.len() < lines {
            self.rewrite(&kept)?;
        }
        Ok(Table::new(kept))
    }

    fn rewrite(&self, kept: &[Sample]) -> Result<(), Fault> {
        let mut text = String::new();
        for sample in kept {
            text.push_str(
                &serde_json::to_string(sample).map_err(|source| Fault::Json { path: self.file.clone(), source })?,
            );
            text.push('\n');
        }
        let io = |source| Fault::Io { path: self.file.clone(), source };
        let temp = self.file.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&temp, text).map_err(io)?;
        std::fs::rename(&temp, &self.file).map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(u64);

    impl Clock for Fixed {
        fn now(&self) -> Moment {
            Moment(self.0)
        }
    }

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("farm-history-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            Self(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn sample(at: u64, host: &str, secs: u64) -> Sample {
        Sample { at: Moment(at), key: Key::stage("test"), host: host.to_owned(), secs, passed: true }
    }

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn a_cargo_key_is_the_subcommand_and_how_many_packages_not_the_command_line() {
        let key = |line: &str| Key::cargo(&args(line));
        assert_eq!(key("clippy -p a --all-targets -- -D warnings"), key("clippy -p b"));
        assert_ne!(key("clippy -p a"), key("clippy -p a -p b"));
        assert_ne!(key("clippy -p a"), key("check -p a"));
        assert_eq!(key("clippy --workspace"), key("clippy"));
        assert_eq!(key("clippy -p a -- -p b"), key("clippy -p z"), "after `--` is the tool's");
        assert_eq!(key("+nightly nextest run -p a").to_string(), "cargo nextest -p");
    }

    #[test]
    fn the_median_is_of_the_passes_of_that_key_and_host_only() {
        let mut samples = vec![sample(1, "a", 10), sample(2, "a", 30), sample(3, "a", 20), sample(4, "b", 99)];
        samples.push(Sample { passed: false, ..sample(5, "a", 1) });
        samples.push(Sample { key: Key::stage("docs"), ..sample(6, "a", 500) });
        let table = Table::new(samples);
        assert_eq!(table.median(&Key::stage("test"), "a"), Some(Median { secs: 20, runs: 3 }));
        assert_eq!(table.median(&Key::stage("test"), "c"), None);
        assert!(table.knows("b") && !table.knows("c"));
        let keys: Vec<String> = table.medians("a").iter().map(|(key, _)| key.to_string()).collect();
        assert_eq!(keys, ["docs", "test"]);
    }

    #[test]
    fn only_the_latest_passes_count() {
        let mut samples: Vec<Sample> = (0..15).map(|at| sample(at, "a", 100)).collect();
        samples.extend((100..115).map(|at| sample(at, "a", 10)));
        assert_eq!(Table::new(samples).median(&Key::stage("test"), "a"), Some(Median { secs: 10, runs: 15 }));
    }

    #[test]
    fn the_file_is_bounded_by_age_and_count_and_rewritten_once_trimmed() {
        let dir = Dir::new("bounds");
        let memory = Memory { hours: NonZeroU64::new(1).unwrap(), samples: NonZeroU32::new(2).unwrap() };
        let history = History::at(&dir.0, memory);
        for at in [1000, 8000, 8100, 8200] {
            history.append(&sample(at, "a", at)).unwrap();
        }
        let now = 8200 + 100;
        let table = history.load(&Fixed(now)).unwrap();
        assert_eq!(table.samples.iter().map(|s| s.at.0).collect::<Vec<_>>(), [8100, 8200]);
        let text = std::fs::read_to_string(&history.file).unwrap();
        assert_eq!(text.lines().count(), 2, "the file shrank to what is kept");
        assert_eq!(history.load(&Fixed(now)).unwrap(), table);
        let aged = history.load(&Fixed(8100 + 3650)).unwrap();
        assert_eq!(aged.samples.iter().map(|s| s.at.0).collect::<Vec<_>>(), [8200]);
    }

    #[test]
    fn a_missing_file_is_no_history_and_a_cut_line_is_skipped() {
        let dir = Dir::new("cut");
        let history = History::at(&dir.0, Memory::default());
        assert_eq!(history.load(&Fixed(5)).unwrap(), Table::default());
        history.append(&sample(1, "a", 3)).unwrap();
        let mut file = std::fs::OpenOptions::new().append(true).open(&history.file).unwrap();
        write!(file, "{{\"at\":2,\"key\":{{\"ki").unwrap();
        assert_eq!(history.load(&Fixed(5)).unwrap().samples.len(), 1);
    }
}
