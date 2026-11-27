//! A host as the config describes it, and as a probe finds it.

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;
use std::time::Duration;

use crate::farm::name::{Destination, HostName, JobId, JobFault};

/// A CPU architecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Arch {
    X86,
    Aarch64,
    /// Any other, as `uname -m` says it.
    Other(String),
}

impl Arch {
    /// The architecture the repo's goldens are blessed on (docs/conventions.md, Tests).
    pub(crate) const REFERENCE: Self = Self::X86;
}

impl FromStr for Arch {
    type Err = std::convert::Infallible;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        Ok(match given {
            "x86_64" | "amd64" => Self::X86,
            "aarch64" | "arm64" => Self::Aarch64,
            other => Self::Other(other.to_owned()),
        })
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86 => out.write_str("x86_64"),
            Self::Aarch64 => out.write_str("aarch64"),
            Self::Other(other) => out.write_str(other),
        }
    }
}

/// How a host is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Via {
    /// This machine, through `sh`, in the workspace itself.
    Local,
    /// Another machine, through `ssh` and `rsync`.
    Ssh(Destination),
}

impl FromStr for Via {
    type Err = crate::farm::name::Fault;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        match given {
            "local" => Ok(Self::Local),
            other => other.parse().map(Self::Ssh),
        }
    }
}

/// A directory on a host: absolute, or relative to the home of the user that logs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostDir(String);

impl HostDir {
    /// The default, `~/farm`.
    pub(crate) fn standard() -> Self {
        Self("farm".to_owned())
    }

    /// The path of `parts` below it, with `/` between.
    pub(crate) fn join(&self, parts: &[&str]) -> String {
        let mut path = self.0.clone();
        for part in parts {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
        }
        path
    }

    /// The lock file of slot `n`.
    #[cfg(test)]
    pub(crate) fn slot(&self, n: u32) -> String {
        self.join(&["slots", &format!("{n}.lock")])
    }

    /// The directory as an argument to a program.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for HostDir {
    type Err = DirFault;

    fn from_str(given: &str) -> Result<Self, DirFault> {
        let path = match given {
            "~" => "",
            other => other.strip_prefix("~/").unwrap_or(other),
        };
        let bad = |why| DirFault { given: given.to_owned(), why };
        if path.is_empty() && given != "~" {
            return Err(bad(Why::Empty));
        }
        if let Some(found) = path.chars().find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))) {
            return Err(bad(Why::Char(found)));
        }
        if path.split('/').any(|part| part == ".." || part == ".") {
            return Err(bad(Why::Dots));
        }
        Ok(Self(path.trim_end_matches('/').to_owned()))
    }
}

impl fmt::Display for HostDir {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_str() {
            "" => out.write_str("~"),
            path if path.starts_with('/') => out.write_str(path),
            path => write!(out, "~/{path}"),
        }
    }
}

/// A host directory that is not allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirFault {
    given: String,
    why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Why {
    Empty,
    Char(char),
    Dots,
}

impl fmt::Display for DirFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let given = &self.given;
        match self.why {
            Why::Empty => write!(out, "the host directory is empty"),
            Why::Char(found) => write!(out, "host directory `{given}` holds {found:?}; use letters, digits and `/ . _ -`"),
            Why::Dots => write!(out, "host directory `{given}` holds a `.` or `..` part"),
        }
    }
}

impl std::error::Error for DirFault {}

/// A host from the config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Host {
    pub(crate) name: HostName,
    pub(crate) via: Via,
    /// Slots by config; `None` is cores divided by six, at least one.
    pub(crate) slots: Option<u32>,
    /// Arch by config; `None` is what the probe finds.
    pub(crate) arch: Option<Arch>,
    /// Whether goldens may run here by config; `None` is whether the arch is the reference.
    pub(crate) golden: Option<bool>,
    pub(crate) mold: bool,
    pub(crate) dir: HostDir,
}

impl Host {
    /// This machine, with every default.
    pub(crate) fn local() -> Self {
        Self {
            name: HostName::local(),
            via: Via::Local,
            slots: None,
            arch: None,
            golden: None,
            mold: false,
            dir: HostDir::standard(),
        }
    }
}

/// A load average, in hundredths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Load(pub(crate) u32);

impl FromStr for Load {
    type Err = ParseIntError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
        let mut hundredths: String = frac.chars().take(2).collect();
        while hundredths.len() < 2 {
            hundredths.push('0');
        }
        let whole: u32 = whole.parse()?;
        Ok(Self(whole.saturating_mul(100).saturating_add(hundredths.parse()?)))
    }
}

impl fmt::Display for Load {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

/// One job a host lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobRow {
    pub(crate) id: JobId,
    pub(crate) state: JobState,
    /// Seconds since the epoch: when it started, was queued or ended.
    pub(crate) at: u64,
    pub(crate) shown: String,
}

/// Where a listed job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobState {
    Queued,
    Running,
    /// Ended with `code` after `ran`.
    Done { code: i32, ran: Duration },
    /// Its body died without a result.
    Lost,
}

/// What a probe found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) name: HostName,
    pub(crate) arch: Arch,
    pub(crate) golden: bool,
    pub(crate) cores: u32,
    pub(crate) load: Load,
    pub(crate) slots: u32,
    pub(crate) busy: u32,
    pub(crate) queued: u32,
    pub(crate) now: u64,
    pub(crate) jobs: Vec<JobRow>,
}

impl Snapshot {
    /// Whether a slot is free and nothing waits for it.
    pub(crate) fn is_free(&self) -> bool {
        self.busy < self.slots && self.queued == 0
    }

    /// Takes a slot, or a place in the queue, for a job about to be placed here.
    pub(crate) fn reserve(&mut self) {
        if self.is_free() {
            self.busy += 1;
        } else {
            self.queued += 1;
        }
    }

    /// The cores each slot's job may use: cargo's `-j` and nextest's threads.
    pub(crate) fn share(&self) -> u32 {
        (self.cores / self.slots.max(1)).max(1)
    }

    /// About how long until a slot frees, from the jobs this host finished lately: the mean of
    /// their run times, less the age of the oldest running job. `None` with no history.
    pub(crate) fn eta(&self) -> Option<Duration> {
        let ran: Vec<u64> = self
            .jobs
            .iter()
            .filter_map(|job| match job.state {
                JobState::Done { code: 0, ran } => Some(ran.as_secs()),
                _ => None,
            })
            .collect();
        let mean = ran.iter().sum::<u64>().checked_div(u64::try_from(ran.len()).ok()?)?;
        let oldest = self
            .jobs
            .iter()
            .filter(|job| job.state == JobState::Running)
            .map(|job| self.now.saturating_sub(job.at))
            .max()
            .unwrap_or(0);
        Some(Duration::from_secs(mean.saturating_sub(oldest).max(5)))
    }

    /// Reads a probe's output for `host`.
    pub(crate) fn read(host: &Host, text: &str) -> Result<Self, ReadFault> {
        let mut found = Found::default();
        for line in text.lines() {
            found.take(line, host)?;
        }
        found.finish(host)
    }
}

/// The lines of a probe, as they are read.
#[derive(Debug, Default)]
struct Found {
    arch: Option<Arch>,
    cores: Option<u32>,
    load: Option<Load>,
    slots: Option<u32>,
    now: Option<u64>,
    queued: u32,
    busy: u32,
    free: u32,
    jobs: Vec<JobRow>,
}

impl Found {
    fn take(&mut self, line: &str, host: &Host) -> Result<(), ReadFault> {
        let int = |text: &str| -> Result<u64, ReadFault> {
            text.parse().map_err(|source| ReadFault::Int { line: line.to_owned(), source })
        };
        let mut words = line.splitn(2, ' ');
        let (key, rest) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
        match key {
            "now" => self.now = Some(int(rest)?),
            "arch" => self.arch = host.arch.clone().or_else(|| rest.parse().ok()),
            "cores" => self.cores = Some(u32::try_from(int(rest)?).unwrap_or(u32::MAX)),
            "slots" => self.slots = Some(u32::try_from(int(rest)?).unwrap_or(u32::MAX)),
            "load" => self.load = Some(rest.parse().map_err(|source| ReadFault::Int { line: line.to_owned(), source })?),
            "queued" => self.queued = u32::try_from(int(rest)?).unwrap_or(u32::MAX),
            "slot" if rest.ends_with(" busy") => self.busy += 1,
            "slot" if rest.ends_with(" free") => self.free += 1,
            "job" => self.jobs.push(JobRow::read(rest).map_err(|fault| ReadFault::Job { line: line.to_owned(), fault })?),
            _ => return Err(ReadFault::Unknown { line: line.to_owned() }),
        }
        Ok(())
    }

    fn finish(self, host: &Host) -> Result<Snapshot, ReadFault> {
        let missing = |key| ReadFault::Missing { key };
        let arch = self.arch.ok_or_else(|| missing("arch"))?;
        let cores = self.cores.ok_or_else(|| missing("cores"))?;
        let slots = host.slots.or(self.slots).ok_or_else(|| missing("slots"))?;
        Ok(Snapshot {
            name: host.name.clone(),
            golden: host.golden.unwrap_or(arch == Arch::REFERENCE),
            arch,
            cores,
            load: self.load.ok_or_else(|| missing("load"))?,
            slots,
            busy: self.busy,
            queued: self.queued,
            now: self.now.ok_or_else(|| missing("now"))?,
            jobs: self.jobs,
        })
    }
}

impl JobRow {
    /// Reads `<id> <state> <epoch> ...`: a finished job's rest is `<rc> <secs> <command>`, any
    /// other's is the command.
    fn read(rest: &str) -> Result<Self, JobReadFault> {
        let mut words = rest.splitn(4, ' ');
        let id: JobId = words.next().unwrap_or("").parse().map_err(JobReadFault::Id)?;
        let state = words.next().unwrap_or("");
        let at: u64 = words.next().unwrap_or("").parse().map_err(JobReadFault::Int)?;
        let tail = words.next().unwrap_or("");
        let (state, shown) = match state {
            "queued" => (JobState::Queued, tail),
            "running" => (JobState::Running, tail),
            "lost" => (JobState::Lost, tail),
            "done" => {
                let mut parts = tail.splitn(3, ' ');
                let code: i32 = parts.next().unwrap_or("").parse().map_err(JobReadFault::Int)?;
                let ran: u64 = parts.next().unwrap_or("").parse().map_err(JobReadFault::Int)?;
                (JobState::Done { code, ran: Duration::from_secs(ran) }, parts.next().unwrap_or(""))
            }
            other => return Err(JobReadFault::State { given: other.to_owned() }),
        };
        Ok(Self { id, state, at, shown: shown.to_owned() })
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let golden = if self.golden { "golden" } else { "not golden" };
        write!(
            out,
            "{}: {} {} cores, load {}, slots {}/{} busy, {} queued, {golden}",
            self.name, self.arch, self.cores, self.load, self.busy, self.slots, self.queued
        )?;
        for job in self.jobs.iter().filter(|job| !matches!(job.state, JobState::Done { .. })) {
            let age = self.now.saturating_sub(job.at);
            let state = match job.state {
                JobState::Queued => "queued",
                JobState::Running => "running",
                JobState::Lost => "lost",
                JobState::Done { .. } => "done",
            };
            write!(out, "\n  {} {state} for {age} s: {}", job.id, job.shown)?;
        }
        Ok(())
    }
}

/// A probe's output that is not what a probe prints.
#[derive(Debug)]
pub(crate) enum ReadFault {
    Unknown { line: String },
    Int { line: String, source: ParseIntError },
    Job { line: String, fault: JobReadFault },
    Missing { key: &'static str },
}

/// A job line that is not one.
#[derive(Debug)]
pub(crate) enum JobReadFault {
    Id(JobFault),
    Int(ParseIntError),
    State { given: String },
}

impl fmt::Display for ReadFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { line } => write!(out, "unexpected probe line `{line}`"),
            Self::Int { line, .. } => write!(out, "probe line `{line}` has a bad number"),
            Self::Job { line, .. } => write!(out, "probe job line `{line}` is not one"),
            Self::Missing { key } => write!(out, "the probe did not say `{key}`"),
        }
    }
}

impl fmt::Display for JobReadFault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(_) => write!(out, "its id is not a job id"),
            Self::Int(_) => write!(out, "it has a bad number"),
            Self::State { given } => write!(out, "`{given}` is not a job state"),
        }
    }
}

impl std::error::Error for ReadFault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Int { source, .. } => Some(source),
            Self::Job { fault, .. } => Some(fault),
            Self::Unknown { .. } | Self::Missing { .. } => None,
        }
    }
}

impl std::error::Error for JobReadFault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Id(source) => Some(source),
            Self::Int(source) => Some(source),
            Self::State { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBE: &str = "now 1700000100
arch x86_64
cores 14
load 3.52
slots 2
slot 0 busy
slot 1 free
queued 3
job petal.1a2 running 1700000040 cargo clippy -p engine
job petal.1a1 done 1700000000 0 85 cargo nextest run
job petal.1a0 done 1699999000 101 40 cargo nextest run
job petal.19f lost 1699990000 cargo check
job petal.19e queued 1699990000 cargo xtask check lint
";

    fn host(name: &str) -> Host {
        Host { name: name.parse().unwrap(), ..Host::local() }
    }

    #[test]
    fn a_probe_reads_into_a_snapshot() {
        assert_eq!("3.52".parse::<Load>().unwrap(), Load(352));
        assert_eq!("0.5".parse::<Load>().unwrap(), Load(50));
        assert_eq!("12".parse::<Load>().unwrap().to_string(), "12.00");
        let snap = Snapshot::read(&host("petal"), PROBE).unwrap();
        assert_eq!((snap.cores, snap.slots, snap.busy, snap.queued), (14, 2, 1, 3));
        assert_eq!(snap.arch, Arch::X86);
        assert!(snap.golden);
        assert_eq!(snap.jobs.len(), 5);
        assert_eq!(snap.jobs.first().map(|job| job.shown.as_str()), Some("cargo clippy -p engine"));
        assert_eq!(snap.jobs.get(1).map(|job| job.state), Some(JobState::Done { code: 0, ran: Duration::from_secs(85) }));
        assert_eq!(snap.share(), 7);
    }

    #[test]
    fn config_overrides_the_probe_and_golden_follows_arch() {
        let arm = PROBE.replace("x86_64", "aarch64");
        assert!(!Snapshot::read(&host("am3"), &arm).unwrap().golden);
        let forced = Host { golden: Some(true), slots: Some(1), ..host("am3") };
        let snap = Snapshot::read(&forced, &arm).unwrap();
        assert!(snap.golden);
        assert_eq!(snap.slots, 1);
        let told = Host { arch: Some(Arch::X86), ..host("am3") };
        assert!(Snapshot::read(&told, &arm).unwrap().golden);
    }

    #[test]
    fn a_bad_probe_is_an_error_not_a_guess() {
        for bad in ["", "now x", "now 1\nwhat 1", "now 1\narch x86_64\ncores 4\nload 1\nslots 1\njob a b c"] {
            assert!(Snapshot::read(&host("petal"), bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_eta_is_the_mean_run_less_the_oldest_age() {
        let snap = Snapshot::read(&host("petal"), PROBE).unwrap();
        // done with code 0: 85 s; running since 60 s before now
        assert_eq!(snap.eta(), Some(Duration::from_secs(25)));
        let idle = Snapshot::read(&host("petal"), "now 1\narch x\ncores 1\nload 0\nslots 1\nslot 0 free\nqueued 0\n").unwrap();
        assert_eq!(idle.eta(), None);
    }

    #[test]
    fn reserving_fills_slots_then_the_queue() {
        let mut snap = Snapshot::read(&host("petal"), "now 1\narch x86_64\ncores 12\nload 0\nslots 2\nslot 0 free\nslot 1 free\nqueued 0\n").unwrap();
        snap.reserve();
        snap.reserve();
        assert_eq!((snap.busy, snap.queued), (2, 0));
        snap.reserve();
        assert_eq!((snap.busy, snap.queued), (2, 1));
    }

    #[test]
    fn directories_are_home_relative_or_absolute_and_never_dotted() {
        assert_eq!("~/farm".parse::<HostDir>().unwrap().join(&["slots", "0.lock"]), "farm/slots/0.lock");
        assert_eq!("/srv/farm/".parse::<HostDir>().unwrap().slot(3), "/srv/farm/slots/3.lock");
        assert_eq!("~".parse::<HostDir>().unwrap().join(&["x"]), "x");
        for bad in ["", "a b", "../x", "a/../b", "a;b", "$HOME", "~/a'b"] {
            assert!(bad.parse::<HostDir>().is_err(), "{bad:?}");
        }
        assert_eq!("~/farm".parse::<HostDir>().unwrap().to_string(), "~/farm");
    }
}
