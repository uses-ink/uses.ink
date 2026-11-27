//! Where a job goes: which hosts may run it, and which of them is the best one now.

use std::fmt;

use crate::error::Error;
use crate::host::Snapshot;
use crate::name::HostName;

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

/// Picks the host for a job from what the probes found. A new policy is a new type.
pub(crate) trait Placement {
    /// The index in `board` of the host to run a job needing `need`; `pin` names the only host
    /// allowed, when the user chose one.
    fn choose(&self, need: Need, pin: Option<&HostName>, board: &[Snapshot]) -> Result<usize, Error>;
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
    fn choose(&self, need: Need, pin: Option<&HostName>, board: &[Snapshot]) -> Result<usize, Error> {
        if let Some(pinned) = pin {
            return match board.iter().position(|snap| &snap.name == pinned) {
                Some(at) if board.get(at).is_some_and(|snap| need.fits(snap)) => Ok(at),
                Some(_) => Err(Error::NotGolden { host: pinned.clone() }),
                None => Err(Error::NoHost { need, up: board.iter().map(|snap| snap.name.clone()).collect() }),
            };
        }
        board
            .iter()
            .enumerate()
            .filter(|(_, snap)| need.fits(snap))
            .min_by_key(|(_, snap)| Rank::of(snap))
            .map(|(at, _)| at)
            .ok_or_else(|| Error::NoHost { need, up: board.iter().map(|snap| snap.name.clone()).collect() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{Arch, Load};

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
        }
    }

    fn pick(need: Need, board: &[Snapshot]) -> Option<&str> {
        LeastLoaded.choose(need, None, board).ok().and_then(|at| board.get(at)).map(|snap| snap.name.as_str())
    }

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
        assert!(matches!(LeastLoaded.choose(Need::Golden, None, &[arm]), Err(Error::NoHost { .. })));
        assert!(matches!(LeastLoaded.choose(Need::Any, None, &[]), Err(Error::NoHost { .. })));
    }

    #[test]
    fn a_pinned_host_is_the_only_one_and_must_fit() {
        let mut arm = snap("arm", [4, 0, 0, 0]);
        arm.golden = false;
        let board = [snap("x86", [1, 0, 0, 0]), arm];
        let arm_name: HostName = "arm".parse().unwrap();
        assert_eq!(LeastLoaded.choose(Need::Any, Some(&arm_name), &board).unwrap(), 1);
        assert!(matches!(LeastLoaded.choose(Need::Golden, Some(&arm_name), &board), Err(Error::NotGolden { .. })));
        let gone: HostName = "gone".parse().unwrap();
        assert!(matches!(LeastLoaded.choose(Need::Any, Some(&gone), &board), Err(Error::NoHost { .. })));
    }

    #[test]
    fn placing_many_jobs_spreads_them_before_it_queues() {
        let mut board = vec![snap("a", [1, 0, 0, 0]), snap("b", [2, 0, 0, 0])];
        let mut got = Vec::new();
        for _ in 0..5 {
            let at = LeastLoaded.choose(Need::Any, None, &board).unwrap();
            got.push(board.get(at).unwrap().name.to_string());
            board.get_mut(at).unwrap().reserve();
        }
        assert_eq!(got[..3].iter().filter(|name| *name == "b").count(), 2);
        assert_eq!(got[..3].iter().filter(|name| *name == "a").count(), 1);
    }
}
