//! The watch on a real control plane, with a remote the test moves and a clock it controls.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise and read top to bottom"
)]

extern crate uses_ink_watch as watch;

use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use blob_dir::Directory;
use build::{Revision, Source};
use clock::{Clock, Counter, Manual};
use control::Ports as _;
use control::{AccountKind, Control, Domain, Id, JobState, NewAccount, NewSite, Origin, Remembered, SiteKind, Treed};
use db::Steps;
use db_sqlite::Sqlite;
use jiff::Timestamp;
use model::ExternalUrl;
use model::git::Locator;
use queue_rows::{Local as Queue, Queued};
use secret_local::Local as Sealer;
use watch::{Done, Head, Heads, Interval, IntervalError, Outcome, Previews, Report, Round, Watch, Worker};

type Ports = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

#[derive(Debug)]
struct Down;

impl fmt::Display for Down {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the remote is down")
    }
}

impl std::error::Error for Down {}

/// A remote whose branch the test moves.
#[derive(Debug, Default)]
struct Remote {
    at: RefCell<Option<Head>>,
    tips: RefCell<Vec<(&'static str, char)>>,
    asked: Cell<u32>,
}

impl Remote {
    fn move_to(&self, digit: char) {
        *self.at.borrow_mut() = Some(digit.to_string().repeat(40).parse().unwrap());
    }
}

impl Heads for &Remote {
    type Error = Down;

    fn head(
        &self,
        _: &Locator,
        _: Option<&Revision>,
        _: Option<&watch::Access>,
    ) -> impl Future<Output = Result<Head, Down>> {
        self.asked.set(self.asked.get() + 1);
        std::future::ready(self.at.borrow().clone().ok_or(Down))
    }

    fn branches(&self, _: &Locator, _: Option<&watch::Access>) -> impl Future<Output = Result<watch::Branches, Down>> {
        let found = self
            .tips
            .borrow()
            .iter()
            .map(|(name, digit)| watch::Tip {
                branch: name.parse().unwrap(),
                head: digit.to_string().repeat(40).parse().unwrap(),
            })
            .collect();
        std::future::ready(Ok(watch::Branches { default: Some("main".parse().unwrap()), found, skipped: Vec::new() }))
    }
}

struct Seen(RefCell<Vec<usize>>);

impl Report<Round> for Seen {
    fn report(&self, round: &Round) {
        self.0.borrow_mut().push(round.polled.len());
    }
}

fn start() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

struct World {
    control: Control<Ports>,
    site: Id<SiteKind>,
    _folder: tempfile::TempDir,
}

impl World {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = control::Schema::after(
            Steps::new()
                .with::<Domain>()
                .unwrap()
                .with::<Remembered>()
                .unwrap()
                .with::<Treed>()
                .unwrap()
                .with::<Queued>()
                .unwrap(),
        )
        .unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (
            db.clone(),
            Directory::open(folder.path().join("b")).unwrap(),
            Queue::new(db),
            Sealer::ephemeral().unwrap(),
        );
        let origin = Origin::try_from(&ExternalUrl::parse("https://uses.ink").unwrap()).unwrap();
        let control = Control::new(ports, origin);
        let account: Id<AccountKind> = Id::from_bytes([1; 16]);
        pollster::block_on(control.add_account(NewAccount { id: account, at: start() })).unwrap();
        let source = Source::Git { url: "https://codeberg.org/a/b".parse().unwrap(), revision: None };
        let site = Id::from_bytes([2; 16]);
        let new = NewSite { id: site, account, handle: "journal".parse().unwrap(), source, at: start() };
        pollster::block_on(control.add_site(new)).unwrap();
        Self { control, site, _folder: folder }
    }

    fn builds(&self) -> usize {
        pollster::block_on(self.control.jobs(self.site, 50)).unwrap().len()
    }
}

#[test]
fn queues_a_build_when_the_source_moves_and_backs_off_when_it_does_not() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    let clock = Manual::new(start(), 100);
    let mut watch = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let step = |watch: &mut Watch<'_, Ports, &Remote, &Manual, Counter>| pollster::block_on(watch.step());
    let first = step(&mut watch);
    assert!(matches!(first.polled[0].outcome, Outcome::Queued { .. }), "{first:?}");
    let jobs = pollster::block_on(world.control.jobs(world.site, 5)).unwrap();
    assert_eq!((jobs[0].trigger, jobs[0].state), (build::Trigger::Poll, JobState::Queued));
    // nothing is due until the interval has passed
    assert!(step(&mut watch).polled.is_empty());
    assert_eq!(remote.asked.get(), 1);
    clock.advance(Duration::from_secs(600));
    assert!(matches!(step(&mut watch).polled[0].outcome, Outcome::Unchanged));
    // unchanged doubled the wait to 20 minutes
    clock.advance(Duration::from_secs(600));
    assert!(step(&mut watch).polled.is_empty());
    clock.advance(Duration::from_secs(600));
    assert!(matches!(step(&mut watch).polled[0].outcome, Outcome::Unchanged));
    // a move queues a build again, and the wait is the base again
    remote.move_to('b');
    clock.advance(Duration::from_secs(3600));
    assert!(matches!(step(&mut watch).polled[0].outcome, Outcome::Queued { .. }));
    assert_eq!(world.builds(), 2);
    clock.advance(Duration::from_secs(600));
    assert_eq!(step(&mut watch).polled.len(), 1);
}

#[test]
fn queues_a_preview_build_for_each_new_branch_once_and_not_for_the_default() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    *remote.tips.borrow_mut() = vec![("main", 'a'), ("feature/login", 'c'), ("zeta", 'd'), ("beta", 'e')];
    let clock = Manual::new(start(), 100);
    let mut watch =
        Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default()).with_previews(Previews(2));
    let first = pollster::block_on(watch.step());
    let named: Vec<String> = first.branched.iter().filter_map(|b| b.branch.as_ref().map(ToString::to_string)).collect();
    assert_eq!(named, ["beta", "feature/login"], "by name, capped at two, the default left out: {first:?}");
    assert!(first.branched.iter().any(|b| matches!(b.outcome, Outcome::Capped { left: 1 })), "{first:?}");
    let jobs = pollster::block_on(world.control.jobs(world.site, 10)).unwrap();
    let pinned: Vec<String> = jobs.iter().filter_map(|j| j.pin.as_ref().map(|p| p.branch.to_string())).collect();
    assert_eq!(jobs.len(), 3, "the site's own build and two previews");
    assert_eq!(pinned.len(), 2, "{pinned:?}");
    // a later poll with the same tips queues nothing new
    clock.advance(Duration::from_secs(3600));
    let again = pollster::block_on(watch.step());
    assert!(again.branched.iter().all(|b| matches!(b.outcome, Outcome::Capped { .. })), "{again:?}");
    assert_eq!(pollster::block_on(world.control.jobs(world.site, 10)).unwrap().len(), 3);
    // a branch that moved is built again
    remote.tips.borrow_mut()[1].1 = 'f';
    clock.advance(Duration::from_secs(3600));
    pollster::block_on(watch.step());
    assert_eq!(pollster::block_on(world.control.jobs(world.site, 10)).unwrap().len(), 4);
}

#[test]
fn a_watch_with_previews_off_lists_no_branches() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    *remote.tips.borrow_mut() = vec![("feature", 'c')];
    let clock = Manual::new(start(), 100);
    let mut watch = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let round = pollster::block_on(watch.step());
    assert!(round.branched.is_empty(), "{round:?}");
    assert_eq!(world.builds(), 1);
}

#[test]
fn a_remote_that_does_not_answer_is_asked_less_often_and_queues_nothing() {
    let world = World::new();
    let remote = Remote::default();
    let clock = Manual::new(start(), 100);
    let mut watch = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let outcome = pollster::block_on(watch.step()).polled.remove(0).outcome;
    assert!(matches!(outcome, Outcome::Failed(_)));
    clock.advance(Duration::from_secs(600));
    assert!(pollster::block_on(watch.step()).polled.is_empty(), "the wait doubled after a failure");
    remote.move_to('c');
    clock.advance(Duration::from_secs(600));
    assert!(matches!(pollster::block_on(watch.step()).polled[0].outcome, Outcome::Queued { .. }));
    assert_eq!(world.builds(), 1);
}

#[test]
fn a_restart_queues_nothing_for_a_source_that_did_not_move() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    let clock = Manual::new(start(), 100);
    let ids = Counter::new();
    let mut first = Watch::new(&world.control, &remote, (&clock, &ids), Interval::default());
    assert!(matches!(pollster::block_on(first.step()).polled[0].outcome, Outcome::Queued { .. }));
    let mut second = Watch::new(&world.control, &remote, (&clock, &ids), Interval::default());
    let outcome = pollster::block_on(second.step()).polled.remove(0).outcome;
    assert!(matches!(outcome, Outcome::Unchanged), "{outcome:?}");
    assert_eq!(world.builds(), 1);
    // moved while the watch was down: the next one queues a build, once
    remote.move_to('b');
    clock.advance(Duration::from_secs(600));
    let mut third = Watch::new(&world.control, &remote, (&clock, &ids), Interval::default());
    let outcome = pollster::block_on(third.step()).polled.remove(0).outcome;
    assert!(matches!(outcome, Outcome::Queued { .. }), "{outcome:?}");
    assert_eq!(world.builds(), 2);
}

#[test]
fn a_queued_build_of_the_same_commit_is_not_doubled_when_the_memory_is_lost() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    let clock = Manual::new(start(), 100);
    let mut first = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    pollster::block_on(first.step());
    let forget = control::table::Seen::Table;
    let wipe = db::sea_query::Query::delete().from_table(forget).to_owned();
    pollster::block_on(db::Db::run(world.control.ports().db(), wipe.into())).unwrap();
    let mut second = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let outcome = pollster::block_on(second.step()).polled.remove(0).outcome;
    assert!(matches!(outcome, Outcome::Duplicate { .. }), "{outcome:?}");
    assert_eq!(world.builds(), 1);
}

#[test]
fn the_loop_rests_between_rounds_and_never_spins() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    let clock = Manual::new(start(), 4);
    let mut watch = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let seen = Seen(RefCell::default());
    pollster::block_on(watch.run(&seen));
    assert_eq!(seen.0.borrow().len(), 5, "four rests, and the fifth says stop");
    assert!(clock.sleeps().iter().all(|rest| *rest >= Duration::from_secs(1)), "{:?}", clock.sleeps());
    assert_eq!(
        clock.sleeps()[0],
        Interval::default().lookout(),
        "a site added meanwhile is noticed within the lookout"
    );
    assert!(clock.now() > start());
}

#[test]
fn an_interval_has_a_floor_and_a_direction() {
    assert!(Interval::new(Duration::from_secs(10), Duration::from_secs(10)).is_ok());
    assert!(matches!(Interval::new(Duration::from_secs(1), Duration::from_secs(60)), Err(IntervalError::Fast { .. })));
    assert!(matches!(Interval::default().with_lookout(Duration::from_millis(10)), Err(IntervalError::Lookout { .. })));
    assert!(matches!(
        Interval::new(Duration::from_secs(60), Duration::from_secs(30)),
        Err(IntervalError::Backwards { .. })
    ));
    let interval = Interval::default();
    assert_eq!(interval.backed(Duration::from_mins(40)), Duration::from_secs(3600));
}

/// A builder that is never asked: the queue is empty in the test that uses it.
struct Idle;

#[derive(Debug)]
struct Never;

impl fmt::Display for Never {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the idle builder was asked to build")
    }
}

impl std::error::Error for Never {}

impl build::Build for Idle {
    type Error = Never;

    fn run(
        &self,
        _: &build::Request,
        _: &mut impl build::Events,
    ) -> impl Future<Output = Result<build::Outcome, build::Error<Never>>> {
        std::future::ready(Err(build::Error::Backend { source: Never }))
    }
}

/// What the worker reported.
struct Told(RefCell<Vec<u64>>);

impl Report<Done> for Told {
    fn report(&self, done: &Done) {
        if let Done::Expired(swept) = done {
            self.0.borrow_mut().push(swept.previews);
        }
    }
}

#[test]
fn an_idle_worker_sweeps_the_previews_that_ended() {
    let world = World::new();
    // a preview of nothing: the preview needs a manifest, not files
    let manifest = model::Manifest::default();
    let new = control::NewPreview { site: world.site, slug: "main".parse().unwrap(), manifest, at: start() };
    pollster::block_on(world.control.preview(new)).unwrap();
    // the clock is a week and a day on from the push when the worker rests for the first time
    let clock = Manual::new(start() + Duration::from_hours(8 * 24), 1);
    let told = Told(RefCell::default());
    pollster::block_on(
        Worker::new(&world.control, Idle, &clock, (Duration::from_secs(60), Duration::from_secs(1))).run(&told),
    );
    assert_eq!(*told.0.borrow(), [1], "swept once, and not again");
    assert_eq!(pollster::block_on(world.control.previews(world.site, clock.now())).unwrap(), []);
}

fn push(branch: &str, digit: char, default: Option<&str>) -> watch::Pushed {
    watch::Pushed {
        branch: branch.parse().unwrap(),
        head: digit.to_string().repeat(40).parse().unwrap(),
        default: default.map(|name| name.parse().unwrap()),
    }
}

/// Takes the one queued request off the queue and settles it, as a worker that built it would.
fn dequeue(world: &World, now: Timestamp) {
    use queue::Queue as _;
    let queue = world.control.ports().queue();
    let delivery = pollster::block_on(queue.receive(now, Duration::from_secs(60))).unwrap().unwrap();
    pollster::block_on(queue.ack(&delivery)).unwrap();
}

#[test]
fn a_push_to_the_default_branch_queues_a_build_that_the_poll_does_not_double() {
    let world = World::new();
    let remote = Remote::default();
    remote.move_to('a');
    let clock = Manual::new(start(), 100);
    let mut watch = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let outcome = pollster::block_on(watch.pushed(world.site, &push("main", 'a', Some("main"))));
    assert!(matches!(outcome, Outcome::Queued { .. }), "{outcome:?}");
    let jobs = pollster::block_on(world.control.jobs(world.site, 5)).unwrap();
    assert_eq!((jobs[0].trigger, jobs[0].state), (build::Trigger::Push, JobState::Queued));
    // the same delivery again, and a redelivery of the same commit, queue nothing
    let again = pollster::block_on(watch.pushed(world.site, &push("main", 'a', Some("main"))));
    assert!(matches!(again, Outcome::Duplicate { .. }), "{again:?}");
    // the build is worked and gone from the queue before the poll first looks: the queue's own
    // dedupe no longer knows the commit, the row of what was seen does
    dequeue(&world, clock.now());
    let round = pollster::block_on(watch.step());
    assert!(matches!(round.polled[0].outcome, Outcome::Unchanged), "{round:?}");
    assert_eq!(world.builds(), 1, "one push and the poll that follows it make one build");
    // a new commit pushed after the poll's memory holds the old one is a second build
    remote.move_to('b');
    let outcome = pollster::block_on(watch.pushed(world.site, &push("main", 'b', Some("main"))));
    assert!(matches!(outcome, Outcome::Queued { .. }), "{outcome:?}");
    dequeue(&world, clock.now());
    clock.advance(Duration::from_secs(3600));
    let round = pollster::block_on(watch.step());
    assert!(matches!(round.polled[0].outcome, Outcome::Unchanged), "{round:?}");
    assert_eq!(world.builds(), 2);
}

#[test]
fn a_push_to_another_branch_is_a_preview_through_the_pin_table_or_is_left_with_a_reason() {
    let world = World::new();
    let remote = Remote::default();
    let clock = Manual::new(start(), 100);
    let off = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let left = pollster::block_on(off.pushed(world.site, &push("feature", 'c', Some("main"))));
    assert!(matches!(left, Outcome::Left(watch::Unplaced::Previews { .. })), "{left:?}");
    let nameless = pollster::block_on(off.pushed(world.site, &push("main", 'c', None)));
    assert!(matches!(nameless, Outcome::Left(watch::Unplaced::Default { .. })), "{nameless:?}");
    assert_eq!(world.builds(), 0);
    let watch =
        Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default()).with_previews(Previews(5));
    let queued = pollster::block_on(watch.pushed(world.site, &push("feature/login", 'c', Some("main"))));
    assert!(matches!(queued, Outcome::Queued { .. }), "{queued:?}");
    let jobs = pollster::block_on(world.control.jobs(world.site, 5)).unwrap();
    assert_eq!(jobs[0].pin.as_ref().map(|pin| pin.branch.to_string()).as_deref(), Some("feature/login"));
    assert_eq!(jobs[0].trigger, build::Trigger::Push);
    // the same branch at the same commit is built once, and a commit that moved builds again
    let again = pollster::block_on(watch.pushed(world.site, &push("feature/login", 'c', Some("main"))));
    assert!(matches!(again, Outcome::Duplicate { .. }), "{again:?}");
    let moved = pollster::block_on(watch.pushed(world.site, &push("feature/login", 'd', Some("main"))));
    assert!(matches!(moved, Outcome::Queued { .. }), "{moved:?}");
    assert_eq!(world.builds(), 2);
}

#[test]
fn a_push_to_a_site_that_does_not_exist_is_a_failure_with_its_cause() {
    let world = World::new();
    let remote = Remote::default();
    let clock = Manual::new(start(), 100);
    let watch = Watch::new(&world.control, &remote, (&clock, Counter::new()), Interval::default());
    let ghost: Id<SiteKind> = Id::from_bytes([77; 16]);
    let outcome = pollster::block_on(watch.pushed(ghost, &push("main", 'a', Some("main"))));
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
}
