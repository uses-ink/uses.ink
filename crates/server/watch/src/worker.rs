//! Working builds off the queue.

use logging::Field as _;
use std::error::Error;
use std::time::Duration;

use build::Build;
use clock::{Clock, Slept};
use control::{Control, Ports, Swept, Worked};

use crate::Report;

/// What one try at the queue came to.
#[derive(Debug)]
pub enum Done {
    /// The queue gave a request and it was worked.
    Worked(Worked),
    /// The control plane or the builder failed; the request goes back to the queue for later.
    Failed(Box<dyn Error>),
    /// The queue was empty and previews that had ended were swept (decision 0091).
    Expired(Swept),
}

/// The in-process worker: takes one request at a time and builds it.
#[derive(Debug)]
pub struct Worker<'c, P: Ports, W, K> {
    control: &'c Control<P>,
    builder: W,
    clock: K,
    lease: Duration,
    idle: Duration,
}

impl<'c, P: Ports + 'static, W: Build, K: Clock> Worker<'c, P, W, K>
where
    W::Error: 'static,
{
    /// A worker that holds a request for `lease` and rests `idle` when there is nothing to do.
    /// A rest under a second is raised to a second: an empty queue is never a busy loop.
    pub fn new(control: &'c Control<P>, builder: W, clock: K, times: (Duration, Duration)) -> Self {
        Self { control, builder, clock, lease: times.0, idle: times.1.max(Duration::from_secs(1)) }
    }

    /// Takes one request off the queue and works it.
    pub async fn step(&self) -> Done {
        match self.control.work(&self.builder, self.clock.now(), self.lease).await {
            Ok(worked) => {
                logging::debug!(
                    name: logging::event::WORKER_TICK,
                    outcome = match &worked {
                        Worked::Idle => "idle",
                        Worked::Built { .. } => "built",
                        Worked::Refused { .. } => "refused",
                        Worked::Previewed { .. } => "previewed",
                        _ => "dropped",
                    },
                );
                Done::Worked(worked)
            }
            Err(error) => {
                logging::error!(name: logging::event::WORKER_TICK, outcome = "failed", error = logging::Failure(&error).field());
                Done::Failed(Box::new(error))
            }
        }
    }

    /// Sweeps the previews that have ended, and says so when there were any. The queue is empty,
    /// so this is the quiet part of the loop, at most once a rest.
    async fn tidy(&self, report: &impl Report<Done>) {
        match self.control.expire(self.clock.now()).await {
            Ok(swept) if swept.previews > 0 => report.report(&Done::Expired(swept)),
            Ok(_) => {}
            Err(error) => report.report(&Done::Failed(Box::new(error))),
        }
    }

    /// Works requests until the queue is empty, rests, and looks again, until the clock says it
    /// is stopped. A failure rests too, so a broken builder is not a busy loop.
    pub async fn run(&self, report: &impl Report<Done>) {
        loop {
            let done = self.step().await;
            report.report(&done);
            let busy = matches!(&done, Done::Worked(worked) if *worked != Worked::Idle);
            if !busy {
                self.tidy(report).await;
                if self.clock.sleep(self.idle).await == Slept::Stopped {
                    return;
                }
            }
        }
    }
}
