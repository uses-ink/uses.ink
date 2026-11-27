//! The loops `ink serve --data` runs beside the server (decision 0067): the watch that polls
//! git sources, and the worker that builds what the queue holds.
//!
//! Each runs on a thread of its own, on a runtime of its own, under a supervisor: a panic or an
//! early return is an event, and the loop is started again after a delay, up to a limit of
//! crashes in a row. A stop is graceful: the loop finishes the step it is in, and its next wait
//! returns as stopped. Everything the loops and their supervisors do is a typed [`Event`] on a
//! channel, which the server prints.

use std::fmt;
use std::panic::AssertUnwindSafe;
use std::time::{Duration, Instant};

use build::BuildId;
use clock::{Clock, Slept, System};
use control::{Dropped, Id, Published, SiteKind, Worked};
use edge_control::Chain;
use tokio::sync::{mpsc, watch as flag};
use trace::Field as _;
use watch::{Done, Head, Outcome, Report, Round};

/// Which loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    /// Polls the git sources.
    Watch,
    /// Builds what the queue holds.
    Worker,
}

impl trace::Kind for Task {
    fn kind(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Worker => "worker",
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Watch => "watch",
            Self::Worker => "worker",
        })
    }
}

/// When a crashed loop is started again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Restart {
    /// How many crashes in a row are survived; the next one leaves the loop stopped.
    pub(crate) limit: u32,
    /// How long to wait before starting again.
    pub(crate) delay: Duration,
    /// How long a run must last for the crashes before it to be forgotten.
    pub(crate) healthy: Duration,
}

impl Default for Restart {
    fn default() -> Self {
        Self { limit: 5, delay: Duration::from_secs(2), healthy: Duration::from_secs(60) }
    }
}

/// Why a loop ended when nobody stopped it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    /// It panicked, saying this.
    Panicked(String),
    /// It returned: a loop that waits forever has no reason to.
    Returned,
    /// The runtime it needs could not be made.
    Runtime(String),
}

impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Panicked(said) => write!(out, "panicked: {said}"),
            Self::Returned => out.write_str("returned"),
            Self::Runtime(said) => write!(out, "no runtime: {said}"),
        }
    }
}

impl std::error::Error for Cause {}

/// What the loops and their supervisors did.
#[derive(Debug)]
pub(crate) enum Event {
    /// A loop started, or started again.
    Started(Task),
    /// The watch queued a build of a commit that moved.
    Queued { site: Id<SiteKind>, build: BuildId, head: Head },
    /// The watch saw a commit that a queued build has already.
    Duplicate { site: Id<SiteKind>, head: Head },
    /// The watch could not read a source's head, or queue its build.
    Unreachable { site: Id<SiteKind>, cause: Chain },
    /// The watch could not list the sites.
    Unlisted { cause: Chain },
    /// The worker built a site.
    Built { build: BuildId, published: Published },
    /// The worker found a site that does not build.
    Refused { build: BuildId },
    /// The worker dropped a request it could not work.
    Dropped(Dropped),
    /// The worker failed on a request and left it on the queue.
    Failed { cause: Chain },
    /// A loop ended without being stopped.
    Crashed { task: Task, attempt: u32, cause: Cause },
    /// A crashed loop starts again after this long.
    Restarting { task: Task, after: Duration },
    /// A loop crashed more often than the limit and is left stopped.
    GaveUp { task: Task, attempts: u32 },
    /// A loop stopped when told to.
    Stopped(Task),
}

impl fmt::Display for Event {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started(task) => write!(out, "{task}: started"),
            Self::Queued { site, build, head } => write!(out, "watch: queued build {build} of site {site} at {head}"),
            Self::Duplicate { site, head } => write!(out, "watch: site {site} at {head} is queued already"),
            Self::Unreachable { site, cause } => write!(out, "watch: site {site}: {}", Self::chain(cause)),
            Self::Unlisted { cause } => write!(out, "watch: sites not listed: {}", Self::chain(cause)),
            Self::Built { build, published } => {
                let said = match published {
                    Published::Live { .. } => "live",
                    Published::Unchanged => "unchanged",
                    _ => "held, not live",
                };
                write!(out, "worker: built {build} ({said})")
            }
            Self::Refused { build } => write!(out, "worker: build {build} refused: the site does not build"),
            Self::Dropped(reason) => write!(out, "worker: dropped a request: {reason:?}"),
            Self::Failed { cause } => write!(out, "worker: failed: {}", Self::chain(cause)),
            Self::Crashed { task, attempt, cause } => write!(out, "{task}: crashed ({attempt}): {cause}"),
            Self::Restarting { task, after } => write!(out, "{task}: restarting in {}s", after.as_secs()),
            Self::GaveUp { task, attempts } => write!(out, "{task}: stopped for good after {attempts} crashes"),
            Self::Stopped(task) => write!(out, "{task}: stopped"),
        }
    }
}

impl Event {
    /// Logs what a supervisor did. The watch's and the worker's own events are logged where they
    /// happen (`watch.polled`, `worker.tick`), so they are not logged again here.
    fn log(&self) {
        match self {
            Self::Started(task) => trace::info!(name: trace::event::TASK_STARTED, task = trace::Kind::kind(*task)),
            Self::Crashed { task, attempt, cause } => trace::error!(
                name: trace::event::TASK_CRASHED,
                task = trace::Kind::kind(*task),
                attempt = trace::Count(u64::from(*attempt)).field(),
                error = trace::Failure(cause).field(),
            ),
            Self::Restarting { task, after } => trace::warn!(
                name: trace::event::TASK_RESTARTING,
                task = trace::Kind::kind(*task),
                after_us = trace::Took(*after).field(),
            ),
            Self::GaveUp { task, attempts } => trace::error!(
                name: trace::event::TASK_GAVE_UP,
                task = trace::Kind::kind(*task),
                attempt = trace::Count(u64::from(*attempts)).field(),
            ),
            Self::Stopped(task) => trace::info!(name: trace::event::TASK_STOPPED, task = trace::Kind::kind(*task)),
            _ => {}
        }
    }

    /// An error and its causes on one line.
    fn chain(error: &Chain) -> String {
        model::Chain::new(error).to_string()
    }
}

/// Where the loops tell what they did.
#[derive(Clone, Debug)]
pub(crate) struct Events(mpsc::UnboundedSender<Event>);

impl Events {
    /// A channel of events.
    pub(crate) fn channel() -> (Self, mpsc::UnboundedReceiver<Event>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self(sender), receiver)
    }

    /// Tells one event, and logs it when it is the supervisor's. A receiver that is gone has
    /// stopped listening, which ends nothing.
    pub(crate) fn tell(&self, event: Event) {
        event.log();
        self.0.send(event).ok();
    }
}

impl Report<Round> for Events {
    fn report(&self, round: &Round) {
        if let Some(error) = &round.listing {
            self.tell(Event::Unlisted { cause: Chain::of(&**error) });
        }
        for polled in &round.polled {
            let site = polled.site;
            match &polled.outcome {
                Outcome::Queued { build, head } => self.tell(Event::Queued { site, build: *build, head: head.clone() }),
                Outcome::Duplicate { head } => self.tell(Event::Duplicate { site, head: head.clone() }),
                Outcome::Failed(error) => self.tell(Event::Unreachable { site, cause: Chain::of(&**error) }),
                _ => {}
            }
        }
    }
}

impl Report<Done> for Events {
    fn report(&self, done: &Done) {
        match done {
            Done::Worked(Worked::Built { build, recorded }) => {
                self.tell(Event::Built { build: *build, published: recorded.published });
            }
            Done::Worked(Worked::Refused { build }) => self.tell(Event::Refused { build: *build }),
            Done::Worked(Worked::Dropped(reason)) => self.tell(Event::Dropped(*reason)),
            Done::Worked(_) => {}
            Done::Failed(error) => self.tell(Event::Failed { cause: Chain::of(&**error) }),
        }
    }
}

/// The system clock, whose waits end as stopped once the loops are told to stop.
#[derive(Clone, Debug)]
pub(crate) struct Stoppable(flag::Receiver<bool>);

impl Clock for Stoppable {
    fn now(&self) -> jiff::Timestamp {
        System.now()
    }

    async fn sleep(&self, time: Duration) -> Slept {
        let mut halt = self.0.clone();
        tokio::select! {
            () = tokio::time::sleep(time) => Slept::Elapsed,
            // a sender that is gone stops the loops as well
            _ = halt.wait_for(|halted| *halted) => Slept::Stopped,
        }
    }
}

/// The loops that run, and the way to stop them.
#[derive(Debug)]
pub(crate) struct Tasks {
    halt: flag::Sender<bool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

/// One loop under its supervisor.
struct Supervised {
    task: Task,
    restart: Restart,
    clock: Stoppable,
    events: Events,
}

impl Supervised {
    /// Runs `body` on a runtime of its own until it is told to stop; starts it again when it
    /// ends otherwise.
    fn supervise<F>(self, body: F)
    where
        F: Fn(Stoppable, Events) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>,
    {
        let mut crashes = 0_u32;
        loop {
            self.events.tell(Event::Started(self.task));
            let began = Instant::now();
            let cause = self.attempt(&body);
            if self.halted() {
                self.events.tell(Event::Stopped(self.task));
                return;
            }
            if began.elapsed() >= self.restart.healthy {
                crashes = 0;
            }
            crashes += 1;
            self.events.tell(Event::Crashed { task: self.task, attempt: crashes, cause });
            if crashes > self.restart.limit {
                self.events.tell(Event::GaveUp { task: self.task, attempts: crashes });
                return;
            }
            self.events.tell(Event::Restarting { task: self.task, after: self.restart.delay });
            if self.rest() {
                self.events.tell(Event::Stopped(self.task));
                return;
            }
        }
    }

    fn halted(&self) -> bool {
        *self.clock.0.borrow()
    }

    fn runtime() -> Result<tokio::runtime::Runtime, Cause> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Cause::Runtime(error.to_string()))
    }

    /// One run of the body, to its end.
    fn attempt<F>(&self, body: &F) -> Cause
    where
        F: Fn(Stoppable, Events) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>,
    {
        let runtime = match Self::runtime() {
            Ok(runtime) => runtime,
            Err(cause) => return cause,
        };
        let ran = std::panic::catch_unwind(AssertUnwindSafe(|| {
            runtime.block_on(body(self.clock.clone(), self.events.clone()));
        }));
        match ran {
            Ok(()) => Cause::Returned,
            Err(payload) => {
                let said = payload
                    .downcast_ref::<&str>()
                    .map(|said| (*said).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "no message".to_owned());
                Cause::Panicked(said)
            }
        }
    }

    /// Waits out the restart delay; whether a stop came meanwhile.
    fn rest(&self) -> bool {
        Self::runtime().is_ok_and(|runtime| runtime.block_on(self.clock.sleep(self.restart.delay)) == Slept::Stopped)
    }
}

impl Tasks {
    /// Starts no loop yet.
    pub(crate) fn new() -> Self {
        Self { halt: flag::channel(false).0, threads: Vec::new() }
    }

    /// Starts `body` as `task` on a thread of its own, supervised by `restart`. `body` builds the
    /// loop's future on that thread, so nothing it holds crosses threads.
    pub(crate) fn start<F>(&mut self, task: Task, (restart, events): (Restart, &Events), body: F)
    where
        F: Fn(Stoppable, Events) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + Send + 'static,
    {
        let supervised = Supervised { task, restart, clock: Stoppable(self.halt.subscribe()), events: events.clone() };
        let named = std::thread::Builder::new().name(format!("ink-{task}"));
        match named.spawn(move || supervised.supervise(body)) {
            Ok(thread) => self.threads.push(thread),
            Err(error) => events.tell(Event::Crashed { task, attempt: 0, cause: Cause::Runtime(error.to_string()) }),
        }
    }

    /// Tells every loop to stop after the step it is in.
    pub(crate) fn halt(&self) {
        self.halt.send_replace(true);
    }

    /// Waits for every loop to end.
    pub(crate) fn join(self) {
        for thread in self.threads {
            // a loop that panicked outside its supervisor has said so already
            thread.join().ok();
        }
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn quick() -> Restart {
        Restart { limit: 2, delay: Duration::from_millis(1), healthy: Duration::from_secs(3600) }
    }

    fn drained(mut receiver: mpsc::UnboundedReceiver<Event>) -> Vec<String> {
        let mut seen = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            seen.push(event.to_string());
        }
        seen
    }

    #[test]
    fn a_loop_that_keeps_crashing_is_left_stopped_after_its_limit() {
        let (events, receiver) = Events::channel();
        let mut tasks = Tasks::new();
        let runs = Arc::new(AtomicU32::new(0));
        let counted = Arc::clone(&runs);
        tasks.start(Task::Worker, (quick(), &events), move |_, _| {
            counted.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { panic!("builder gone") })
        });
        tasks.join();
        assert_eq!(runs.load(Ordering::SeqCst), 3, "the first run and two restarts");
        let seen = drained(receiver);
        assert!(seen.iter().any(|line| line == "worker: crashed (1): panicked: builder gone"), "{seen:?}");
        assert_eq!(seen.last().unwrap(), "worker: stopped for good after 3 crashes");
    }

    #[test]
    fn supervision_is_logged_as_typed_events() {
        let capture = trace::Capture::new();
        let (events, _receiver) = Events::channel();
        // the sender stays alive, so no wait ends as stopped
        let (halt, clock) = flag::channel(false);
        let supervised = Supervised { task: Task::Worker, restart: quick(), clock: Stoppable(clock), events };
        capture.run(|| supervised.supervise(|_, _| Box::pin(async { panic!("builder gone") })));
        drop(halt);
        let names: Vec<_> = capture.events().iter().map(|seen| seen.name).collect();
        let (started, crashed, restarting) =
            (trace::event::TASK_STARTED, trace::event::TASK_CRASHED, trace::event::TASK_RESTARTING);
        assert_eq!(
            names,
            [started, crashed, restarting, started, crashed, restarting, started, crashed, trace::event::TASK_GAVE_UP]
        );
        let ended = capture.named(crashed);
        assert_eq!(ended.iter().map(|seen| seen.number("attempt")).collect::<Vec<_>>(), [Some(1), Some(2), Some(3)]);
        assert_eq!(ended[0].text("task"), Some("worker"));
        assert_eq!(ended[0].level, trace::Level::ERROR);
        assert_eq!(capture.named(restarting)[0].level, trace::Level::WARN);
        assert_eq!(capture.named(trace::event::TASK_GAVE_UP)[0].number("attempt"), Some(3));
    }

    #[test]
    fn a_stop_ends_a_loop_after_its_step_without_a_crash() {
        let (events, receiver) = Events::channel();
        let mut tasks = Tasks::new();
        tasks.start(Task::Watch, (quick(), &events), |clock, _| {
            Box::pin(async move { while clock.sleep(Duration::from_secs(3600)).await == Slept::Elapsed {} })
        });
        tasks.halt();
        tasks.join();
        assert_eq!(drained(receiver), ["watch: started", "watch: stopped"]);
    }
}
