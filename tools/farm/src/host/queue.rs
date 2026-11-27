//! The jobs of a host: its holders, its running and queued rows.

use std::time::Duration;

use crate::cli::name::JobId;
use crate::host::{JobReadFault, Load};

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
    Done {
        code: i32,
        ran: Duration,
    },
    /// Its body died without a result.
    Lost,
}

/// Who holds a busy slot: the body of the job that took it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Holder {
    pub(crate) slot: u32,
    pub(crate) pid: u32,
    /// Seconds since the epoch: when it took the slot.
    pub(crate) since: u64,
    /// Whether that process still runs; if not, something else keeps the lock.
    pub(crate) alive: bool,
}

impl Holder {
    /// How long a slot may be busy on a quiet host before it looks stuck.
    pub(super) const PATIENCE: u64 = 300;
    /// A load under this, in hundredths, is a host doing nothing.
    pub(super) const QUIET: Load = Load(100);

    /// Reads `<slot> <pid> <since> <alive|gone>`.
    pub(super) fn read(rest: &str) -> Option<Self> {
        let mut words = rest.split(' ');
        let slot = words.next()?.parse().ok()?;
        let pid = words.next()?.parse().ok()?;
        let since = words.next()?.parse().ok()?;
        let alive = match words.next()? {
            "alive" => true,
            "gone" => false,
            _ => return None,
        };
        Some(Self { slot, pid, since, alive })
    }
}

impl JobRow {
    /// Reads `<id> <state> <epoch> ...`: a finished job's rest is `<rc> <secs> <command>`, any
    /// other's is the command.
    pub(super) fn read(rest: &str) -> Result<Self, JobReadFault> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{Host, Snapshot};

    const PROBE: [&str; 9] = [
        "now 10000",
        "arch x86_64",
        "cores 12",
        "load 0.20",
        "slots 2",
        "slot 0 busy",
        "holder 0 4242 9000 alive",
        "slot 1 free",
        "queued 0",
    ];

    #[test]
    fn a_busy_slot_shows_its_holder_and_is_wedged_when_old_on_a_quiet_host() {
        let snap = Snapshot::read(&Host::local(), &PROBE.join("\n")).unwrap();
        assert_eq!(snap.holders, [Holder { slot: 0, pid: 4242, since: 9000, alive: true }]);
        let shown = snap.to_string();
        assert!(shown.contains("slot 0 held by pid 4242 for 1000 s") && shown.contains("wedged?"), "{shown}");
        let young = PROBE.join("\n").replace("9000", "9900");
        assert!(!Snapshot::read(&Host::local(), &young).unwrap().to_string().contains("wedged?"));
        let busy = PROBE.join("\n").replace("load 0.20", "load 5.00");
        assert!(
            !Snapshot::read(&Host::local(), &busy).unwrap().to_string().contains("wedged?"),
            "a busy host is working"
        );
        let gone = busy.replace("alive", "gone");
        let shown = Snapshot::read(&Host::local(), &gone).unwrap().to_string();
        assert!(shown.contains("gone") && shown.contains("wedged?"), "{shown}");
        assert!(Snapshot::read(&Host::local(), &PROBE.join("\n").replace("alive", "maybe")).is_err());
    }
}
