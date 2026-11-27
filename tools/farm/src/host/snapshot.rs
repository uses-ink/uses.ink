//! What a probe of a host says, read into a [`Snapshot`].

use std::fmt;
use std::num::{ParseIntError, TryFromIntError};
use std::time::Duration;

use crate::cli::name::{HostName, JobFault};
use crate::host::queue::{Holder, JobRow, JobState};
use crate::host::{Arch, Host, Load};

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
    pub(crate) holders: Vec<Holder>,
    /// Seconds a wake is expected to take, when the host does not answer and can be woken.
    pub(crate) asleep: Option<u64>,
}

impl Snapshot {
    /// Whether a busy slot looks stuck: held more than five minutes on a host whose load is under
    /// 1.0, or by a process that is gone, so a child that outlived its job holds the lock.
    pub(crate) fn wedged(&self, holder: &Holder) -> bool {
        self.now.saturating_sub(holder.since) > Holder::PATIENCE && (self.load < Holder::QUIET || !holder.alive)
    }

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
    holders: Vec<Holder>,
}

impl Found {
    fn take(&mut self, line: &str, host: &Host) -> Result<(), ReadFault> {
        let int = |text: &str| -> Result<u64, ReadFault> {
            text.parse().map_err(|source| ReadFault::Int { line: line.to_owned(), source })
        };
        let count = |text: &str| -> Result<u32, ReadFault> {
            u32::try_from(int(text)?).map_err(|source| ReadFault::Range { line: line.to_owned(), source })
        };
        let mut words = line.splitn(2, ' ');
        let (key, rest) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
        match key {
            "now" => self.now = Some(int(rest)?),
            "arch" => self.arch = host.arch.clone().or_else(|| rest.parse().ok()),
            "cores" => self.cores = Some(count(rest)?),
            "slots" => self.slots = Some(count(rest)?),
            "load" => {
                self.load = Some(rest.parse().map_err(|source| ReadFault::Int { line: line.to_owned(), source })?);
            }
            "queued" => self.queued = count(rest)?,
            "slot" if rest.ends_with(" busy") => self.busy += 1,
            "slot" if rest.ends_with(" free") => self.free += 1,
            "holder" => {
                self.holders.push(Holder::read(rest).ok_or_else(|| ReadFault::Unknown { line: line.to_owned() })?);
            }
            "job" => {
                self.jobs.push(JobRow::read(rest).map_err(|fault| ReadFault::Job { line: line.to_owned(), fault })?);
            }
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
            holders: self.holders,
            asleep: None,
        })
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let golden = if self.golden { "golden" } else { "not golden" };
        if let Some(cost) = self.asleep {
            return write!(
                out,
                "{}: asleep ({} {} cores, {} slots, {golden}, as last seen); waking it costs about {cost} s",
                self.name, self.arch, self.cores, self.slots
            );
        }
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
        for holder in &self.holders {
            let age = self.now.saturating_sub(holder.since);
            write!(out, "\n  slot {} held by pid {} for {age} s", holder.slot, holder.pid)?;
            if !holder.alive {
                write!(out, " (that process is gone: a child of the job keeps the lock)")?;
            }
            if self.wedged(holder) {
                write!(out, " wedged?")?;
            }
        }
        Ok(())
    }
}

/// A probe's output that is not what a probe prints.
#[derive(Debug)]
pub(crate) enum ReadFault {
    Unknown {
        line: String,
    },
    Int {
        line: String,
        source: ParseIntError,
    },
    /// A number that parses but is out of the range its field holds.
    Range {
        line: String,
        source: TryFromIntError,
    },
    Job {
        line: String,
        fault: JobReadFault,
    },
    Missing {
        key: &'static str,
    },
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
            Self::Range { line, .. } => write!(out, "probe line `{line}` has a number out of range"),
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
            Self::Range { source, .. } => Some(source),
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
        assert_eq!(
            snap.jobs.get(1).map(|job| job.state),
            Some(JobState::Done { code: 0, ran: Duration::from_secs(85) })
        );
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
    fn a_count_out_of_its_range_is_refused_not_clamped() {
        let read = Snapshot::read(&host("petal"), "now 1\nqueued 4294967296");
        assert!(matches!(read, Err(ReadFault::Range { .. })), "{read:?}");
    }

    #[test]
    fn the_eta_is_the_mean_run_less_the_oldest_age() {
        let snap = Snapshot::read(&host("petal"), PROBE).unwrap();
        // done with code 0: 85 s; running since 60 s before now
        assert_eq!(snap.eta(), Some(Duration::from_secs(25)));
        let idle =
            Snapshot::read(&host("petal"), "now 1\narch x\ncores 1\nload 0\nslots 1\nslot 0 free\nqueued 0\n").unwrap();
        assert_eq!(idle.eta(), None);
    }

    #[test]
    fn reserving_fills_slots_then_the_queue() {
        let mut snap = Snapshot::read(
            &host("petal"),
            "now 1\narch x86_64\ncores 12\nload 0\nslots 2\nslot 0 free\nslot 1 free\nqueued 0\n",
        )
        .unwrap();
        snap.reserve();
        snap.reserve();
        assert_eq!((snap.busy, snap.queued), (2, 0));
        snap.reserve();
        assert_eq!((snap.busy, snap.queued), (2, 1));
    }
}
