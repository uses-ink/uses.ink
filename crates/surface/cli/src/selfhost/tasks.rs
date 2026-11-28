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
use logging::Field as _;

use crate::output::term::Line;
use crate::selfhost::report::log::{Component, Datum, Entry, Fact, Happens, Level, Remedy};
use tokio::sync::{mpsc, watch as flag};
use watch::{Done, Head, Outcome, Report, Round};

/// Which loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    /// Polls the git sources.
    Watch,
    /// Builds what the queue holds.
    Worker,
    /// Reads the proof record of each custom domain again (decision 0151).
    Domains,
}

impl logging::Kind for Task {
    fn kind(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Worker => "worker",
            Self::Domains => "domains",
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Watch => "watch",
            Self::Worker => "worker",
            Self::Domains => "domains",
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
    /// The watch queued a preview build of a branch.
    BranchQueued { site: Id<SiteKind>, branch: String, build: BuildId },
    /// The watch could not list a site's branches, or queue a preview of one.
    BranchUnreachable { site: Id<SiteKind>, branch: Option<String>, cause: Chain },
    /// The watch left branches out because the site has more than it previews.
    BranchesCapped { site: Id<SiteKind>, left: usize },
    /// The watch could not list the sites.
    Unlisted { cause: Chain },
    /// The worker built a site.
    Built { build: BuildId, published: Published },
    /// The worker built a branch and recorded its preview.
    Previewed { build: BuildId, slug: String },
    /// The worker found a site that does not build.
    Refused { build: BuildId },
    /// The worker dropped a request it could not work.
    Dropped(Dropped),
    /// The worker failed on a request and left it on the queue.
    Failed { cause: Chain },
    /// The worker swept previews that had ended.
    Expired { previews: u64, blobs: u64 },
    /// The domain loop read proof records again and something changed or could not be read.
    Rechecked(authority_dns::Rechecked),
    /// A loop ended without being stopped.
    Crashed { task: Task, attempt: u32, cause: Cause },
    /// A crashed loop starts again after this long.
    Restarting { task: Task, after: Duration },
    /// A loop crashed more often than the limit and is left stopped.
    GaveUp { task: Task, attempts: u32 },
    /// A loop stopped when told to.
    Stopped(Task),
}

impl From<Task> for Component {
    fn from(task: Task) -> Self {
        match task {
            Task::Watch => Self::Watch,
            Task::Worker => Self::Worker,
            Task::Domains => Self::Domains,
        }
    }
}

impl Event {
    /// A length as whole milliseconds, kept in a fact.
    fn millis(length: Duration) -> Datum {
        Datum::Millis(u64::try_from(length.as_millis()).unwrap_or(u64::MAX))
    }

    /// The facts of a site and its commit.
    fn site(site: Id<SiteKind>) -> Fact {
        Fact::pair("site", Datum::Text(site.to_string()))
    }
}

impl Happens for Event {
    fn level(&self) -> Level {
        match self {
            Self::Crashed { .. } | Self::GaveUp { .. } | Self::Failed { .. } => Level::Error,
            Self::Unreachable { .. }
            | Self::BranchUnreachable { .. }
            | Self::BranchesCapped { .. }
            | Self::Unlisted { .. }
            | Self::Refused { .. }
            | Self::Dropped(_)
            | Self::Restarting { .. } => Level::Warn,
            _ => Level::Info,
        }
    }

    fn component(&self) -> Component {
        match self {
            Self::Started(task) | Self::Stopped(task) => Component::from(*task),
            Self::Crashed { task, .. } | Self::Restarting { task, .. } | Self::GaveUp { task, .. } => {
                Component::from(*task)
            }
            Self::Queued { .. }
            | Self::Duplicate { .. }
            | Self::Unreachable { .. }
            | Self::BranchQueued { .. }
            | Self::BranchUnreachable { .. }
            | Self::BranchesCapped { .. }
            | Self::Unlisted { .. } => Component::Watch,
            Self::Built { .. }
            | Self::Previewed { .. }
            | Self::Refused { .. }
            | Self::Dropped(_)
            | Self::Failed { .. }
            | Self::Expired { .. } => Component::Worker,
            Self::Rechecked(_) => Component::Domains,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Started(_) => "task.started",
            Self::Queued { .. } => "watch.queued",
            Self::Duplicate { .. } => "watch.duplicate",
            Self::Unreachable { .. } => "watch.unreachable",
            Self::BranchQueued { .. } => "watch.branch_queued",
            Self::BranchUnreachable { .. } => "watch.branch_unreachable",
            Self::BranchesCapped { .. } => "watch.branches_capped",
            Self::Unlisted { .. } => "watch.unlisted",
            Self::Built { .. } => "worker.built",
            Self::Previewed { .. } => "worker.previewed",
            Self::Refused { .. } => "worker.refused",
            Self::Dropped(_) => "worker.dropped",
            Self::Failed { .. } => "worker.failed",
            Self::Expired { .. } => "worker.expired",
            Self::Rechecked(_) => "domains.rechecked",
            Self::Crashed { .. } => "task.crashed",
            Self::Restarting { .. } => "task.restarting",
            Self::GaveUp { .. } => "task.gave_up",
            Self::Stopped(_) => "task.stopped",
        }
    }

    fn message(&self) -> Line {
        Line::plain(match self {
            Self::Started(_) => "started".to_owned(),
            Self::Queued { .. } => "queued build".to_owned(),
            Self::Duplicate { .. } => "commit is queued already".to_owned(),
            Self::Unreachable { .. } => "source not read".to_owned(),
            Self::BranchQueued { .. } => "queued preview build".to_owned(),
            Self::BranchUnreachable { .. } => "branch not read".to_owned(),
            Self::BranchesCapped { .. } => "more branches than the previews limit".to_owned(),
            Self::Unlisted { .. } => "sites not listed".to_owned(),
            Self::Built { .. } => "built".to_owned(),
            Self::Previewed { .. } => "built a preview".to_owned(),
            Self::Refused { .. } => "build refused: the site does not build".to_owned(),
            Self::Dropped(_) => "dropped a request".to_owned(),
            Self::Failed { .. } => "failed".to_owned(),
            Self::Expired { .. } => "swept ended previews and unused blobs".to_owned(),
            Self::Rechecked(found) => found.to_string(),
            Self::Crashed { .. } => "crashed".to_owned(),
            Self::Restarting { .. } => "restarting".to_owned(),
            Self::GaveUp { .. } => "stopped for good".to_owned(),
            Self::Stopped(_) => "stopped".to_owned(),
        })
    }

    fn facts(&self) -> Vec<Fact> {
        let text = |key, value: String| Fact::pair(key, Datum::Text(value));
        match self {
            Self::Started(_) | Self::Stopped(_) | Self::Unlisted { .. } | Self::Failed { .. } | Self::Rechecked(_) => {
                Vec::new()
            }
            Self::Queued { site, build, head } => {
                vec![text("build", build.to_string()), Self::site(*site), text("head", head.to_string())]
            }
            Self::Duplicate { site, head } => vec![Self::site(*site), text("head", head.to_string())],
            Self::Unreachable { site, .. } => vec![Self::site(*site)],
            Self::BranchQueued { site, branch, build } => {
                vec![text("build", build.to_string()), text("branch", branch.clone()), Self::site(*site)]
            }
            Self::BranchUnreachable { site, branch, .. } => {
                let mut facts = vec![Self::site(*site)];
                facts.extend(branch.iter().map(|branch| text("branch", branch.clone())));
                facts
            }
            Self::BranchesCapped { site, left } => {
                vec![Self::site(*site), Fact::pair("over", Datum::Count(u64::try_from(*left).unwrap_or(u64::MAX)))]
            }
            Self::Built { build, published } => {
                let state = match published {
                    Published::Live { .. } => "live",
                    Published::Unchanged => "unchanged",
                    _ => "held",
                };
                vec![text("build", build.to_string()), text("state", state.to_owned())]
            }
            Self::Previewed { build, slug } => vec![text("build", build.to_string()), text("preview", slug.clone())],
            Self::Refused { build } => vec![text("build", build.to_string())],
            Self::Dropped(reason) => vec![text("reason", format!("{reason:?}"))],
            Self::Expired { previews, blobs } => {
                vec![Fact::pair("previews", Datum::Count(*previews)), Fact::pair("blobs", Datum::Count(*blobs))]
            }
            Self::Crashed { attempt, .. } => vec![Fact::pair("attempt", Datum::Count(u64::from(*attempt)))],
            Self::Restarting { after, .. } => vec![Fact::pair("after", Self::millis(*after))],
            Self::GaveUp { attempts, .. } => vec![Fact::pair("attempts", Datum::Count(u64::from(*attempts)))],
        }
    }

    fn causes(&self) -> Vec<String> {
        match self {
            Self::Unreachable { cause, .. }
            | Self::BranchUnreachable { cause, .. }
            | Self::Unlisted { cause }
            | Self::Failed { cause } => Entry::chain(cause),
            Self::Crashed { cause, .. } => vec![cause.to_string()],
            _ => Vec::new(),
        }
    }

    fn remedy(&self) -> Option<Remedy> {
        matches!(self, Self::GaveUp { .. }).then_some(Remedy::Restart)
    }
}

impl fmt::Display for Event {
    /// The component, the message and the facts, as one line of text.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}: {}", self.component(), self.message().text())?;
        self.facts().iter().try_for_each(|fact| write!(out, " {}={}", fact.key, fact.text()))
    }
}

impl Event {
    /// Logs what a supervisor did. The watch's and the worker's own events are logged where they
    /// happen (`watch.polled`, `worker.tick`), so they are not logged again here.
    fn log(&self) {
        match self {
            Self::Started(task) => {
                logging::info!(name: logging::event::TASK_STARTED, task = logging::Kind::kind(*task));
            }
            Self::Crashed { task, attempt, cause } => logging::error!(
                name: logging::event::TASK_CRASHED,
                task = logging::Kind::kind(*task),
                attempt = logging::Count(u64::from(*attempt)).field(),
                error = logging::Failure(cause).field(),
            ),
            Self::Restarting { task, after } => logging::warn!(
                name: logging::event::TASK_RESTARTING,
                task = logging::Kind::kind(*task),
                after_us = logging::Took(*after).field(),
            ),
            Self::GaveUp { task, attempts } => logging::error!(
                name: logging::event::TASK_GAVE_UP,
                task = logging::Kind::kind(*task),
                attempt = logging::Count(u64::from(*attempts)).field(),
            ),
            Self::Stopped(task) => {
                logging::info!(name: logging::event::TASK_STOPPED, task = logging::Kind::kind(*task));
            }
            _ => {}
        }
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
        for branched in &round.branched {
            self.branched(branched);
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

impl Events {
    /// Tells what the watch did about the branches of a polled site.
    fn branched(&self, branched: &watch::Branched) {
        let site = branched.site;
        let name = branched.branch.as_ref().map(ToString::to_string);
        match (&branched.outcome, name) {
            (Outcome::Queued { build, .. }, Some(branch)) => {
                self.tell(Event::BranchQueued { site, branch, build: *build });
            }
            (Outcome::Failed(error), branch) => {
                self.tell(Event::BranchUnreachable { site, branch, cause: Chain::of(&**error) });
            }
            (Outcome::Capped { left }, _) => self.tell(Event::BranchesCapped { site, left: *left }),
            _ => {}
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
            Done::Worked(Worked::Previewed { build, slug }) => {
                self.tell(Event::Previewed { build: *build, slug: slug.to_string() });
            }
            Done::Worked(_) => {}
            Done::Failed(error) => self.tell(Event::Failed { cause: Chain::of(&**error) }),
            Done::Expired(swept) => self.tell(Event::Expired { previews: swept.previews, blobs: swept.blobs }),
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
        assert!(seen.iter().any(|line| line == "worker: crashed attempt=1"), "{seen:?}");
        assert_eq!(seen.last().unwrap(), "worker: stopped for good attempts=3");
    }

    #[test]
    fn supervision_is_logged_as_typed_events() {
        let capture = logging::Capture::new();
        let (events, _receiver) = Events::channel();
        // the sender stays alive, so no wait ends as stopped
        let (halt, clock) = flag::channel(false);
        let supervised = Supervised { task: Task::Worker, restart: quick(), clock: Stoppable(clock), events };
        capture.run(|| supervised.supervise(|_, _| Box::pin(async { panic!("builder gone") })));
        drop(halt);
        let names: Vec<_> = capture.events().iter().map(|seen| seen.name).collect();
        let (started, crashed, restarting) =
            (logging::event::TASK_STARTED, logging::event::TASK_CRASHED, logging::event::TASK_RESTARTING);
        assert_eq!(
            names,
            [
                started,
                crashed,
                restarting,
                started,
                crashed,
                restarting,
                started,
                crashed,
                logging::event::TASK_GAVE_UP
            ]
        );
        let ended = capture.named(crashed);
        assert_eq!(ended.iter().map(|seen| seen.number("attempt")).collect::<Vec<_>>(), [Some(1), Some(2), Some(3)]);
        assert_eq!(ended[0].text("task"), Some("worker"));
        assert_eq!(ended[0].level, logging::Level::ERROR);
        assert_eq!(capture.named(restarting)[0].level, logging::Level::WARN);
        assert_eq!(capture.named(logging::event::TASK_GAVE_UP)[0].number("attempt"), Some(3));
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
