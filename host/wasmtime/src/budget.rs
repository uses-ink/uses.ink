//! What one plugin call may spend (architecture.md section 13): fuel, wall time through epochs,
//! and memory and table size per instance. Each limit hit is a typed [`Failure`](crate::Failure).
//! The operator of the build sets a budget; a site's `ink.toml` may only lower it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use model::Bytes;

/// The limits of one call into a plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Wasm fuel: about one unit per instruction.
    pub fuel: u64,
    /// Wall time, measured in [`Budget::tick`]s of the epoch.
    pub time: Duration,
    /// Linear memory of one instance.
    pub memory: Bytes,
    /// Elements of one table of an instance (function references, mostly).
    pub table: u32,
    /// How often the epoch advances: the grain of the time budget, counted in nanoseconds. A tick
    /// of zero is refused when the runtime is made ([`crate::Error::Tick`]), and a source's terms
    /// must carry the runtime's own tick ([`crate::Error::TermsTick`]).
    pub tick: Duration,
}

impl Budget {
    /// The epoch ticks a call may run for, counted in nanoseconds, at least one; none when the
    /// tick is zero.
    pub(crate) fn ticks(&self) -> Option<u64> {
        let ticks = self.time.as_nanos().checked_div(self.tick.as_nanos())?;
        Some(u64::try_from(ticks).unwrap_or(u64::MAX).max(1))
    }

    /// The memory limit as wasmtime counts it.
    pub(crate) fn bytes(&self) -> usize {
        usize::try_from(self.memory.count()).unwrap_or(usize::MAX)
    }
}

impl Default for Budget {
    /// 2 s and 64 MiB, as architecture.md section 13 says; ten billion units of fuel, about a
    /// thousand times what a page of the quiet theme takes; tables of 65,536 elements; an epoch
    /// every 5 ms.
    fn default() -> Self {
        Self {
            fuel: 10_000_000_000,
            time: Duration::from_secs(2),
            memory: Bytes(64 << 20),
            table: 1 << 16,
            tick: Duration::from_millis(5),
        }
    }
}

/// A thread advancing an engine's epoch every [`Budget::tick`], stopped when dropped.
#[derive(Debug)]
pub(crate) struct Ticker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Ticker {
    pub(crate) fn start(engine: wasmtime::Engine, tick: Duration) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                std::thread::sleep(tick);
                engine.increment_epoch();
            }
        });
        Self { stop, thread: Some(thread) }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // A ticker that panicked has nothing left to stop.
            let _joined = thread.join();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_default_counts_four_hundred_ticks_of_two_seconds() {
        assert_eq!(Budget::default().ticks(), Some(400));
    }

    #[test]
    fn a_time_under_one_tick_is_one_tick_and_a_zero_tick_counts_nothing() {
        let short = Budget { time: Duration::from_millis(1), ..Budget::default() };
        assert_eq!(short.ticks(), Some(1));
        let still = Budget { tick: Duration::ZERO, ..Budget::default() };
        assert_eq!(still.ticks(), None);
    }

    #[test]
    fn a_tick_that_is_not_whole_milliseconds_divides_exactly() {
        let odd = Budget { time: Duration::from_secs(3), tick: Duration::from_micros(1500), ..Budget::default() };
        assert_eq!(odd.ticks(), Some(2000));
        let fine = Budget { tick: Duration::from_micros(500), ..Budget::default() };
        assert_eq!(fine.ticks(), Some(4000));
    }

    #[test]
    fn a_runtime_refuses_a_zero_tick() {
        let refused = crate::Runtime::new(Budget { tick: Duration::ZERO, ..Budget::default() }, None).unwrap_err();
        assert!(matches!(refused, crate::Error::Tick { tick } if tick.is_zero()), "{refused:?}");
    }
}
