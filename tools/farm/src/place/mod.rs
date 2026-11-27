//! Where a job goes: which hosts may run it, and which of them is the best one now.

pub(crate) mod history;
pub(crate) mod wake;

use std::fmt;

use crate::cli::error::Error;
use crate::cli::name::HostName;
use crate::host::Snapshot;
use crate::place::history::{Key, Median, Table};

/// What a host must be for a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Need {
    /// Any host.
    Any,
    /// A golden host: output-affecting tests and goldens run only on the reference architecture,
    /// or on one the config declares golden.
    Golden,
}

impl Need {
    /// Whether the host `snap` describes may run the job.
    pub(crate) const fn fits(self, snap: &Snapshot) -> bool {
        match self {
            Self::Any => true,
            Self::Golden => snap.golden,
        }
    }
}

impl fmt::Display for Need {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Any => "any",
            Self::Golden => "golden",
        })
    }
}

/// What a job asks of placement.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ask<'a> {
    pub(crate) need: Need,
    /// What the job is, for its history.
    pub(crate) key: &'a Key,
    /// The only host allowed, when the user chose one.
    pub(crate) pin: Option<&'a HostName>,
}

/// How an estimate came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Basis {
    /// The host's own median for the key.
    Median,
    /// The other hosts' medians for the key, scaled by core count.
    Scaled,
}

/// When a host is expected to finish a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Estimate {
    pub(crate) host: HostName,
    /// Seconds from now to the end of the job, wait for a slot included.
    pub(crate) secs: u64,
    /// Jobs that must finish before a slot is free for it.
    pub(crate) queue: u32,
    pub(crate) basis: Basis,
    /// Seconds of `secs` that are the host starting up; zero for a host that is up.
    pub(crate) wake: u64,
}

impl fmt::Display for Estimate {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scaled = if self.basis == Basis::Scaled { ", scaled from other hosts" } else { "" };
        let wake = if self.wake > 0 { format!(", of which {} s to wake it", self.wake) } else { String::new() };
        write!(out, "{} (est {} s, queue {}{scaled}{wake})", self.host, self.secs, self.queue)
    }
}

/// Why a host was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Why {
    /// `--host` named it.
    Pinned,
    /// No other host may run the job.
    Only,
    /// No host has a time for this job yet: the least loaded.
    Load,
    /// The host has no history at all and a slot is free: it is tried once.
    Trying,
    /// The lowest expected finish, over the runner-up.
    Estimated { best: Estimate, over: Option<Estimate> },
}

/// The host chosen, as an index into the board, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) at: usize,
    pub(crate) host: HostName,
    pub(crate) why: Why,
}

impl fmt::Display for Choice {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host = &self.host;
        match &self.why {
            Why::Pinned => write!(out, "{host} (pinned with --host)"),
            Why::Only => write!(out, "{host} (the only host that may run it)"),
            Why::Load => write!(out, "{host} (no timings for it yet; the least loaded)"),
            Why::Trying => write!(out, "{host} (no history on this host yet; trying it once)"),
            Why::Estimated { best, over: None } => write!(out, "{best}"),
            Why::Estimated { best, over: Some(over) } => {
                let queue = if over.queue > 0 { format!(", queue {}", over.queue) } else { String::new() };
                write!(out, "{best} over {} (est {} s{queue})", over.host, over.secs)
            }
        }
    }
}

/// Picks the host for a job from what the probes found. A new policy is a new type.
pub(crate) trait Placement {
    /// The host to run the job `ask` describes.
    fn choose(&self, ask: Ask<'_>, board: &[Snapshot]) -> Result<Choice, Error>;

    /// The typical times of `host` by key, as placement uses them; none for a policy without.
    fn medians(&self, _host: &HostName) -> Vec<(Key, Median)> {
        Vec::new()
    }
}

impl Ask<'_> {
    fn none(self, board: &[Snapshot]) -> Error {
        Error::NoHost { need: self.need, up: board.iter().map(|snap| snap.name.clone()).collect() }
    }

    /// The pinned host, when there is one: it must be up and fit.
    fn pinned(self, board: &[Snapshot]) -> Option<Result<Choice, Error>> {
        let pinned = self.pin?;
        Some(match board.iter().position(|snap| &snap.name == pinned) {
            Some(at) if board.get(at).is_some_and(|snap| self.need.fits(snap)) => {
                Ok(Choice { at, host: pinned.clone(), why: Why::Pinned })
            }
            Some(_) => Err(Error::NotGolden { host: pinned.clone() }),
            None => Err(self.none(board)),
        })
    }

    /// The hosts that may run the job, with their places on the board.
    fn fits(self, board: &[Snapshot]) -> Vec<(usize, &Snapshot)> {
        board.iter().enumerate().filter(|(_, snap)| self.need.fits(snap)).collect()
    }
}

/// The host with a free slot and the fewest slots in use, then the lowest load; with every slot
/// taken, the one whose queue is shortest for its slots.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LeastLoaded;

/// How good a host is for the next job: smaller is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    /// A slot is free and no one waits.
    Free { used: u32, load: u32 },
    /// Every slot is taken: `waiting` jobs per thousand slots ahead of the job.
    Full { waiting: u32, load: u32 },
}

impl Rank {
    fn of(snap: &Snapshot) -> Self {
        let load = snap.load.0;
        if snap.is_free() {
            Self::Free { used: snap.busy, load }
        } else {
            Self::Full { waiting: (snap.queued + snap.busy) * 1000 / snap.slots.max(1), load }
        }
    }
}

impl Placement for LeastLoaded {
    fn choose(&self, ask: Ask<'_>, board: &[Snapshot]) -> Result<Choice, Error> {
        if let Some(pinned) = ask.pinned(board) {
            return pinned;
        }
        let fits = ask.fits(board);
        // a sleeping host costs money: only with no host up that may run the job
        let up: Vec<_> = fits.iter().copied().filter(|(_, snap)| snap.asleep.is_none()).collect();
        let among = if up.is_empty() { fits } else { up };
        among
            .into_iter()
            .min_by_key(|(_, snap)| Rank::of(snap))
            .map(|(at, snap)| Choice { at, host: snap.name.clone(), why: Why::Load })
            .ok_or_else(|| ask.none(board))
    }
}

/// The host that will finish the job first, by what past runs took: the wait for a free slot
/// (the jobs ahead times the host's median for the key) plus the median. A key a host has never
/// run is estimated from the other hosts' medians scaled by core count; a host with no history
/// at all is tried once while a slot is free; with no timings anywhere it is [`LeastLoaded`].
/// A sleeping host adds its wake cost to its estimate, and is never tried for exploration.
#[derive(Debug, Clone, Default)]
pub(crate) struct Fastest {
    table: Table,
}

/// A host's expected finish, and how good the host is otherwise, as a sort key.
type Score = (u64, bool, Rank, usize);

impl Fastest {
    /// A host whose load is over this share of its cores, in percent, is slowed by the excess.
    const OVERLOAD: u64 = 150;

    pub(crate) const fn new(table: Table) -> Self {
        Self { table }
    }

    /// The typical seconds of `key` on `snap`'s host: its own median, else the other hosts'
    /// medians scaled by core count (fewer cores take longer), else none.
    fn typical(&self, key: &Key, snap: &Snapshot, board: &[Snapshot]) -> Option<(u64, Basis)> {
        if let Some(own) = self.table.median(key, snap.name.as_str()) {
            return Some((own.secs, Basis::Median));
        }
        let mut scaled: Vec<u64> = board
            .iter()
            .filter(|other| other.name != snap.name)
            .filter_map(|other| {
                let median = self.table.median(key, other.name.as_str())?;
                Some(median.secs * u64::from(other.cores.max(1)) / u64::from(snap.cores.max(1)))
            })
            .collect();
        scaled.sort_unstable();
        scaled.get(scaled.len() / 2).map(|secs| (*secs, Basis::Scaled))
    }

    /// When the job would finish on `snap`: wait, run, and the slowdown of an overloaded host.
    fn estimate(snap: &Snapshot, typical: (u64, Basis)) -> Estimate {
        let (secs, basis) = typical;
        let slots = u64::from(snap.slots.max(1));
        let ahead = if snap.is_free() { 0 } else { (snap.busy + snap.queued + 1).saturating_sub(snap.slots).max(1) };
        let total = secs + (u64::from(ahead) * secs).div_ceil(slots);
        let cores = u64::from(snap.cores.max(1)) * 100;
        let load = u64::from(snap.load.0);
        let total = if load * 100 > cores * Self::OVERLOAD { total * load / cores } else { total };
        let wake = snap.asleep.unwrap_or(0);
        Estimate { host: snap.name.clone(), secs: total + wake, queue: ahead, basis, wake }
    }
}

impl Placement for Fastest {
    fn choose(&self, ask: Ask<'_>, board: &[Snapshot]) -> Result<Choice, Error> {
        if let Some(pinned) = ask.pinned(board) {
            return pinned;
        }
        let fits = ask.fits(board);
        if let [(at, snap)] = fits.as_slice() {
            return Ok(Choice { at: *at, host: snap.name.clone(), why: Why::Only });
        }
        let cold = |snap: &Snapshot| snap.is_free() && snap.asleep.is_none() && !self.table.knows(snap.name.as_str());
        if let Some((at, snap)) = fits.iter().find(|(_, snap)| cold(snap)) {
            return Ok(Choice { at: *at, host: snap.name.clone(), why: Why::Trying });
        }
        let mut scored: Vec<(Score, Estimate)> = fits
            .iter()
            .filter_map(|(at, snap)| {
                let estimate = Self::estimate(snap, self.typical(ask.key, snap, board)?);
                Some(((estimate.secs, snap.asleep.is_some(), Rank::of(snap), *at), estimate))
            })
            .collect();
        scored.sort_by_key(|entry| entry.0);
        let mut ranked = scored.into_iter();
        let Some(((.., at), best)) = ranked.next() else {
            return LeastLoaded.choose(ask, board);
        };
        Ok(Choice { at, host: best.host.clone(), why: Why::Estimated { best, over: ranked.next().map(|next| next.1) } })
    }

    fn medians(&self, host: &HostName) -> Vec<(Key, Median)> {
        self.table.medians(host.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Moment;
    use crate::host::{Arch, Load};
    use crate::place::history::Sample;

    fn snap(name: &str, [slots, busy, queued, load]: [u32; 4]) -> Snapshot {
        Snapshot {
            name: name.parse().unwrap(),
            arch: Arch::X86,
            golden: true,
            cores: slots * 6,
            load: Load(load),
            slots,
            busy,
            queued,
            now: 0,
            jobs: Vec::new(),
            holders: Vec::new(),
            asleep: None,
        }
    }

    fn stage() -> Key {
        Key::stage("test")
    }

    fn ask(need: Need, key: &Key) -> Ask<'_> {
        Ask { need, key, pin: None }
    }

    fn pick(need: Need, board: &[Snapshot]) -> Option<&str> {
        let key = stage();
        LeastLoaded.choose(ask(need, &key), board).ok().and_then(|choice| board.get(choice.at)).map(|s| s.name.as_str())
    }

    /// A history of passes: `(host, key, seconds)` repeated three times.
    fn table(times: &[(&str, &str, u64)]) -> Table {
        let mut samples = Vec::new();
        for (at, (host, name, secs)) in times.iter().cycle().take(times.len() * 3).enumerate() {
            samples.push(Sample {
                at: Moment(at as u64),
                key: Key::stage(name),
                host: (*host).to_owned(),
                secs: *secs,
                passed: true,
            });
        }
        Table::new(samples)
    }

    fn fastest(times: &[(&str, &str, u64)], board: &[Snapshot], key: &str) -> Choice {
        Fastest::new(table(times)).choose(ask(Need::Any, &Key::stage(key)), board).unwrap()
    }

    /// The measured farm: `here` has 14 cores and 2 slots, `petal` 8 cores and 1 slot.
    fn farm(here: [u32; 4], petal: [u32; 4]) -> Vec<Snapshot> {
        let mut board = vec![snap("here", here), snap("petal", petal)];
        board[0].cores = 14;
        board[1].cores = 8;
        board
    }

    const TIMES: [(&str, &str, u64); 4] =
        [("here", "test", 64), ("petal", "test", 30), ("here", "lint", 73), ("petal", "lint", 51)];

    #[test]
    fn a_free_host_beats_a_full_one() {
        let board = [snap("full", [1, 1, 0, 10]), snap("free", [2, 1, 0, 4000])];
        assert_eq!(pick(Need::Any, &board), Some("free"));
    }

    #[test]
    fn among_free_hosts_the_fewest_slots_in_use_then_the_lowest_load() {
        let board = [snap("a", [4, 2, 0, 100]), snap("b", [4, 1, 0, 900]), snap("c", [4, 1, 0, 200])];
        assert_eq!(pick(Need::Any, &board), Some("c"));
    }

    #[test]
    fn a_queue_makes_a_host_not_free_even_with_a_slot_showing_free() {
        let board = [snap("queued", [2, 1, 2, 0]), snap("busy", [1, 1, 0, 500])];
        assert_eq!(pick(Need::Any, &board), Some("busy"));
    }

    #[test]
    fn with_everything_full_the_shortest_queue_per_slot_wins() {
        let board = [snap("a", [1, 1, 3, 0]), snap("b", [4, 4, 4, 0]), snap("c", [2, 2, 5, 0])];
        assert_eq!(pick(Need::Any, &board), Some("b"));
    }

    #[test]
    fn golden_jobs_skip_hosts_that_are_not_golden() {
        let mut arm = snap("arm", [4, 0, 0, 0]);
        arm.golden = false;
        arm.arch = Arch::Aarch64;
        let board = [arm.clone(), snap("x86", [1, 1, 6, 900])];
        assert_eq!(pick(Need::Golden, &board), Some("x86"));
        assert_eq!(pick(Need::Any, &board), Some("arm"));
        let key = stage();
        assert!(matches!(LeastLoaded.choose(ask(Need::Golden, &key), &[arm]), Err(Error::NoHost { .. })));
        assert!(matches!(LeastLoaded.choose(ask(Need::Any, &key), &[]), Err(Error::NoHost { .. })));
    }

    #[test]
    fn a_pinned_host_is_the_only_one_and_must_fit() {
        let mut arm = snap("arm", [4, 0, 0, 0]);
        arm.golden = false;
        let board = [snap("x86", [1, 0, 0, 0]), arm];
        let key = stage();
        let arm_name: HostName = "arm".parse().unwrap();
        let gone: HostName = "gone".parse().unwrap();
        let pinned = |need, name| Ask { need, key: &key, pin: Some(name) };
        for placement in [&LeastLoaded as &dyn Placement, &Fastest::new(table(&TIMES))] {
            assert_eq!(placement.choose(pinned(Need::Any, &arm_name), &board).unwrap().at, 1);
            let err = placement.choose(pinned(Need::Golden, &arm_name), &board);
            assert!(matches!(err, Err(Error::NotGolden { .. })));
            assert!(matches!(placement.choose(pinned(Need::Any, &gone), &board), Err(Error::NoHost { .. })));
        }
    }

    #[test]
    fn placing_many_jobs_spreads_them_before_it_queues() {
        let mut board = vec![snap("a", [1, 0, 0, 0]), snap("b", [2, 0, 0, 0])];
        let mut got = Vec::new();
        let key = stage();
        for _ in 0..5 {
            let at = LeastLoaded.choose(ask(Need::Any, &key), &board).unwrap().at;
            got.push(board.get(at).unwrap().name.to_string());
            board.get_mut(at).unwrap().reserve();
        }
        assert_eq!(got[..3].iter().filter(|name| *name == "b").count(), 2);
        assert_eq!(got[..3].iter().filter(|name| *name == "a").count(), 1);
    }

    #[test]
    fn the_host_with_the_lower_median_wins_when_both_are_idle() {
        let choice = fastest(&TIMES, &farm([2, 0, 0, 0], [1, 0, 0, 0]), "test");
        assert_eq!(choice.host.as_str(), "petal");
        assert_eq!(choice.to_string(), "petal (est 30 s, queue 0) over here (est 64 s)");
        assert_eq!(fastest(&TIMES, &farm([2, 0, 0, 0], [1, 0, 0, 0]), "lint").host.as_str(), "petal");
    }

    #[test]
    fn a_wait_for_a_slot_is_the_jobs_ahead_times_the_median() {
        // petal's one slot is busy: 30 s wait + 30 s run = 60 s, still under here's 64 s
        let busy = fastest(&TIMES, &farm([2, 0, 0, 0], [1, 1, 0, 100]), "test");
        assert_eq!(busy.to_string(), "petal (est 60 s, queue 1) over here (est 64 s)");
        // one more queued: 90 s, so the idle host wins
        let queued = fastest(&TIMES, &farm([2, 0, 0, 0], [1, 1, 1, 100]), "test");
        assert_eq!(queued.to_string(), "here (est 64 s, queue 0) over petal (est 90 s, queue 2)");
        // here full on two slots waits half a median for the next job to end: 32 + 64 s
        let full = fastest(&TIMES, &farm([2, 2, 0, 100], [1, 1, 1, 100]), "test");
        assert_eq!(full.to_string(), "petal (est 90 s, queue 2) over here (est 96 s, queue 1)");
    }

    #[test]
    fn a_host_far_over_its_cores_is_slowed_by_the_excess() {
        // petal has 8 cores and a load of 24: three times over, so 30 s becomes 90 s
        let choice = fastest(&TIMES, &farm([2, 0, 0, 0], [1, 0, 0, 2400]), "test");
        assert_eq!(choice.host.as_str(), "here");
        let calm = fastest(&TIMES, &farm([2, 0, 0, 0], [1, 0, 0, 1000]), "test");
        assert_eq!(calm.host.as_str(), "petal", "a busy but not overloaded host is not penalised");
    }

    #[test]
    fn an_unknown_key_is_scaled_from_the_other_hosts_by_core_count() {
        // `docs` was only timed on petal (8 cores, 44 s): here has 14 cores, so 44 * 8 / 14 = 25
        let times = [("petal", "docs", 44), ("here", "test", 64), ("petal", "test", 30)];
        let choice = fastest(&times, &farm([2, 0, 0, 0], [1, 0, 0, 0]), "docs");
        assert_eq!(choice.to_string(), "here (est 25 s, queue 0, scaled from other hosts) over petal (est 44 s)");
    }

    #[test]
    fn a_host_with_no_history_is_tried_once_while_a_slot_is_free() {
        let mut board = farm([2, 0, 0, 0], [1, 0, 0, 0]);
        board.push(snap("am3", [1, 0, 0, 0]));
        let first = fastest(&TIMES, &board, "test");
        assert_eq!((first.host.as_str(), &first.why), ("am3", &Why::Trying));
        board[2].reserve();
        let next = fastest(&TIMES, &board, "test");
        assert_eq!(next.host.as_str(), "petal", "a busy cold host is not waited for");
    }

    #[test]
    fn golden_jobs_only_consider_golden_hosts_even_when_another_is_faster() {
        let mut board = farm([2, 0, 0, 0], [1, 0, 0, 0]);
        board.push(snap("am3", [1, 0, 0, 0]));
        board[2].golden = false;
        let times = [("here", "test", 64), ("petal", "test", 30), ("am3", "test", 1)];
        let table = Fastest::new(table(&times));
        let key = stage();
        let golden = table.choose(ask(Need::Golden, &key), &board).unwrap();
        assert_eq!(golden.host.as_str(), "petal");
        assert_eq!(table.choose(ask(Need::Any, &key), &board).unwrap().host.as_str(), "am3");
    }

    #[test]
    fn with_no_timings_anywhere_it_is_the_least_loaded_and_with_one_fit_it_says_so() {
        let board = farm([2, 1, 0, 100], [1, 0, 0, 100]);
        let times = [("here", "lint", 5), ("petal", "lint", 5)];
        let choice = fastest(&times, &board, "test");
        assert_eq!((choice.host.as_str(), &choice.why), ("petal", &Why::Load));
        let lone = Fastest::new(Table::default()).choose(ask(Need::Any, &stage()), &board[..1]).unwrap();
        assert_eq!(lone.why, Why::Only);
    }

    #[test]
    fn the_choice_is_deterministic_and_ties_go_to_the_idler_host_then_the_earlier_one() {
        let times = [("a", "test", 10), ("b", "test", 10)];
        let board = [snap("a", [2, 1, 0, 0]), snap("b", [2, 0, 0, 0])];
        for _ in 0..3 {
            assert_eq!(fastest(&times, &board, "test").host.as_str(), "b");
        }
        let even = [snap("a", [2, 0, 0, 0]), snap("b", [2, 0, 0, 0])];
        assert_eq!(fastest(&times, &even, "test").host.as_str(), "a");
    }

    fn sleeper(name: &str, cost: u64) -> Snapshot {
        let mut snap = snap(name, [2, 0, 0, 0]);
        snap.asleep = Some(cost);
        snap
    }

    #[test]
    fn a_sleeping_host_is_used_only_when_waking_it_still_finishes_first() {
        let times = [("here", "test", 200), ("farm-a", "test", 100)];
        let key = stage();
        let fast = Fastest::new(table(&times));
        // idle `here` takes 200 s; farm-a takes 100 s + 120 s to wake: 220 s, so stay
        let board = [snap("here", [2, 0, 0, 0]), sleeper("farm-a", 120)];
        assert_eq!(fast.choose(ask(Need::Any, &key), &board).unwrap().host.as_str(), "here");
        // with here queued behind two jobs the estimate is 800 s: wake
        let busy = [snap("here", [1, 1, 2, 0]), sleeper("farm-a", 120)];
        let choice = fast.choose(ask(Need::Any, &key), &busy).unwrap();
        assert_eq!(
            choice.to_string(),
            "farm-a (est 220 s, queue 0, of which 120 s to wake it) over here (est 800 s, queue 3)"
        );
        // a short job is never worth waking for: 10 s on here beats 5 + 120 on farm-a
        let short = Fastest::new(table(&[("here", "test", 10), ("farm-a", "test", 5)]));
        assert_eq!(short.choose(ask(Need::Any, &key), &board).unwrap().host.as_str(), "here");
    }

    #[test]
    fn a_sleeping_host_is_never_explored_and_never_picked_without_timings() {
        let key = stage();
        let board = [snap("here", [2, 1, 0, 100]), sleeper("farm-a", 120)];
        let knows_here = Fastest::new(table(&[("here", "lint", 5)]));
        let choice = knows_here.choose(ask(Need::Any, &key), &board).unwrap();
        assert_eq!((choice.host.as_str(), &choice.why), ("here", &Why::Load));
        let none = Fastest::new(Table::default());
        assert_eq!(none.choose(ask(Need::Any, &key), &board).unwrap().host.as_str(), "here");
        // when nothing else may run the job, the sleeper is the only choice
        let mut arm = snap("arm", [1, 0, 0, 0]);
        arm.golden = false;
        let golden = none.choose(ask(Need::Golden, &key), &[arm, sleeper("farm-a", 120)]).unwrap();
        assert_eq!((golden.host.as_str(), &golden.why), ("farm-a", &Why::Only));
        let least = LeastLoaded.choose(ask(Need::Any, &key), &[sleeper("farm-a", 120)]).unwrap();
        assert_eq!(least.host.as_str(), "farm-a");
    }

    #[test]
    fn a_fan_out_spreads_by_estimate_as_slots_fill() {
        let mut board = farm([2, 0, 0, 0], [1, 0, 0, 0]);
        let fast = Fastest::new(table(&TIMES));
        let key = stage();
        let mut hosts = Vec::new();
        for _ in 0..3 {
            let choice = fast.choose(ask(Need::Any, &key), &board).unwrap();
            hosts.push(choice.host.to_string());
            board[choice.at].reserve();
        }
        assert_eq!(hosts, ["petal", "petal", "here"], "the second still beats 64 s at 60 s; the third waits 90 s");
    }
}
