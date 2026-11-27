//! Placement by load alone: the least loaded host that fits.

use crate::cli::error::Error;
use crate::host::Snapshot;
use crate::place::{Ask, Choice, Placement, Why};

/// The host with a free slot and the fewest slots in use, then the lowest load; with every slot
/// taken, the one whose queue is shortest for its slots.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LeastLoaded;

/// How good a host is for the next job: smaller is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Rank {
    /// A slot is free and no one waits.
    Free { used: u32, load: u32 },
    /// Every slot is taken: `waiting` jobs per thousand slots ahead of the job.
    Full { waiting: u32, load: u32 },
}

impl Rank {
    pub(super) fn of(snap: &Snapshot) -> Self {
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
        let fits = ask.fits(board)?;
        // a sleeping host costs money: only with no host up that may run the job; a handicapped
        // one is the last resort among those
        let up: Vec<_> = fits.iter().copied().filter(|(_, snap)| snap.asleep.is_none()).collect();
        let even: Vec<_> = up.iter().copied().filter(|(_, snap)| snap.handicap == 0).collect();
        let up = if even.is_empty() { up } else { even };
        let among = if up.is_empty() { fits } else { up };
        among
            .into_iter()
            .min_by_key(|(_, snap)| Rank::of(snap))
            .map(|(at, snap)| Choice { at, host: snap.name.clone(), why: Why::Load })
            .ok_or_else(|| ask.none(board))
    }
}
