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
use watch::{Head, Heads, Interval, IntervalError, Outcome, Report, Round, Watch};

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
