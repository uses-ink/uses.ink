//! What one plugin call may spend (architecture.md section 13): fuel, wall time through epochs,
//! and memory per instance. Each limit hit is a typed [`Failure`](crate::Failure).

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
    /// Wall time, measured in [`Budget::TICK`]s of the epoch.
    pub time: Duration,
    /// Linear memory of one instance.
    pub memory: Bytes,
}

impl Budget {
    /// How often the epoch advances: the grain of the time budget.
    pub const TICK: Duration = Duration::from_millis(5);

    /// The epoch ticks a call may run for, at least one.
    pub(crate) fn ticks(&self) -> u64 {
        let ticks = self.time.as_millis() / Self::TICK.as_millis();
        u64::try_from(ticks).unwrap_or(u64::MAX).max(1)
    }

    /// The memory limit as wasmtime counts it.
    pub(crate) fn bytes(&self) -> usize {
        usize::try_from(self.memory.count()).unwrap_or(usize::MAX)
    }
}

impl Default for Budget {
    /// 2 s and 64 MiB, as architecture.md section 13 says; ten billion units of fuel, about a
    /// thousand times what a page of the quiet theme takes.
    fn default() -> Self {
        Self { fuel: 10_000_000_000, time: Duration::from_secs(2), memory: Bytes(64 << 20) }
    }
}

/// A thread advancing an engine's epoch every [`Budget::TICK`], stopped when dropped.
#[derive(Debug)]
pub(crate) struct Ticker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Ticker {
    pub(crate) fn start(engine: wasmtime::Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                std::thread::sleep(Budget::TICK);
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
