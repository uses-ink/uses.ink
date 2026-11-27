//! Placement by timing: the host that finishes the job first, from the history.

use crate::cli::error::Error;
use crate::cli::name::HostName;
use crate::host::Snapshot;
use crate::place::history::{Key, Median, Table};
use crate::place::least::{LeastLoaded, Rank};
use crate::place::{Ask, Basis, Choice, Estimate, Placement, Why};

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
