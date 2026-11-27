//! Build jobs, with a builder that does what the test says.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

use blob::{Blob, Chunks, Deleted, Listed, Stored, Tenant};
use blob_dir::{Directory, Error as Fault, Reading};
use build::{Build, BuildId, Built, Event, Events, Outcome, Request, Source, Status, Trigger};
use control::{Dropped, JobState, NewAccount, NewJob, NewSite, Ports, Published, Requested, Worked};
use model::{Bytes, Hash, Manifest, MediaType, Route, Served};
use queue::Dedupe;

use crate::world::{World, at};

#[derive(Debug)]
pub(crate) struct Broke;

impl fmt::Display for Broke {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the builder broke")
    }
}

impl std::error::Error for Broke {}

/// What a scripted build does.
pub(crate) enum Step {
    Build(&'static str),
    Refuse,
    Break,
}

/// A builder that follows a script, storing its blobs in the world's folder like a real one.
pub(crate) struct Scripted {
    store: Directory,
    steps: RefCell<VecDeque<Step>>,
    seen: RefCell<Vec<Request>>,
}

impl Scripted {
    pub(crate) fn new(world: &World, steps: impl IntoIterator<Item = Step>) -> Self {
        Self::in_folder(&world.root, steps)
    }

    fn in_folder(root: &std::path::Path, steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            store: Directory::open(root).unwrap(),
            steps: RefCell::new(steps.into_iter().collect()),
            seen: RefCell::default(),
        }
    }
}

impl Build for Scripted {
    type Error = Broke;

    async fn run(&self, request: &Request, events: &mut impl Events) -> Result<Outcome, build::Error<Broke>> {
        self.seen.borrow_mut().push(request.clone());
        events.event(Event::Started { build: request.build, trigger: request.trigger });
        let step = self.steps.borrow_mut().pop_front().unwrap();
        let (outcome, status) = match step {
            Step::Build(text) => {
                let hash = Hash::of(text);
                self.store.put(&request.tenant, hash, [text.as_bytes()]).await.unwrap();
                let served = Served::new(Route::root(), hash, MediaType::Html, vec![]);
                events.event(Event::Rendered { files: 1, bytes: Bytes(text.len() as u64) });
                let manifest = Manifest::new(vec![served]).unwrap();
                (Outcome::Built(Built { manifest, bytes: Bytes(text.len() as u64) }), Status::Succeeded)
            }
            Step::Refuse => (Outcome::Refused, Status::Refused),
            Step::Break => {
                events.event(Event::Finished { status: Status::Failed, wall: Duration::ZERO });
                return Err(build::Error::Backend { source: Broke });
            }
        };
        events.event(Event::Finished { status, wall: Duration::ZERO });
        Ok(outcome)
    }
}

pub(crate) fn request(site: &control::Site, n: i64, key: Option<&str>) -> (BuildId, NewJob) {
    let id = BuildId::from_bytes([u8::try_from(n).unwrap(); 16]);
    let dedupe = key.map(|key| key.parse::<Dedupe>().unwrap());
    (id, NewJob { id, site: site.id, trigger: Trigger::Push, dedupe, at: at(n) })
}

pub(crate) const LEASE: Duration = Duration::from_secs(60);

#[test]
fn builds_a_requested_site_publishes_it_and_keeps_the_log_as_ndjson() {
    let world = World::new();
    let site = world.site("cstef");
    let (id, new) = request(&site, 10, None);
    assert_eq!(world.run(world.control.request_build(new)).unwrap(), Requested::Queued(id));
    assert_eq!(world.run(world.control.job(id)).unwrap().unwrap().state, JobState::Queued);
    let builder = Scripted::new(&world, [Step::Build("hello")]);
    let worked = world.run(world.control.work(&builder, at(11), LEASE)).unwrap();
    let Worked::Built { build, recorded } = worked else { panic!("{worked:?}") };
    assert_eq!((build, recorded.published), (id, Published::Live { previous: None }));
    let request = builder.seen.borrow().first().cloned().unwrap();
    assert_eq!((request.tenant, request.source), (Tenant::try_from(site.id).unwrap(), site.source.clone()));
    let job = world.run(world.control.job(id)).unwrap().unwrap();
    assert_eq!((job.state, job.deploy, job.finished), (JobState::Succeeded, Some(recorded.deploy), Some(at(11))));
    // the log is NDJSON in the site's own blobs, ending with the publication the builder cannot say
    let tenant = Tenant::try_from(site.id).unwrap();
    let log = world.run(world.control.ports().blob().get(&tenant, job.log.unwrap())).unwrap().unwrap();
    let log = String::from_utf8(world.run(log.all()).unwrap()).unwrap();
    let kinds: Vec<String> = log
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["event"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["started", "rendered", "finished", "published"]);
    assert_eq!(
        world.run(world.control.work(&builder, at(12), LEASE)).unwrap(),
        Worked::Idle,
        "the request was settled"
    );
    assert_eq!(world.run(world.control.jobs(site.id, 5)).unwrap().len(), 1);
}

#[test]
fn a_site_that_does_not_build_is_refused_and_not_retried() {
    let world = World::new();
    let site = world.site("cstef");
    let (id, new) = request(&site, 10, None);
    world.run(world.control.request_build(new)).unwrap();
    let builder = Scripted::new(&world, [Step::Refuse]);
    assert_eq!(world.run(world.control.work(&builder, at(11), LEASE)).unwrap(), Worked::Refused { build: id });
    let job = world.run(world.control.job(id)).unwrap().unwrap();
    assert_eq!((job.state, job.deploy), (JobState::Refused, None));
    assert!(job.log.is_some());
    assert_eq!(world.run(world.control.live(site.id)).unwrap(), None);
    assert_eq!(world.run(world.control.work(&builder, at(500), LEASE)).unwrap(), Worked::Idle);
}

#[test]
fn a_broken_builder_is_retried_then_given_up_on() {
    let world = World::new();
    let site = world.site("cstef");
    let (id, new) = request(&site, 10, None);
    world.run(world.control.request_build(new)).unwrap();
    let builder = Scripted::new(&world, [Step::Break, Step::Break, Step::Break]);
    for (n, now) in [(1, 11), (2, 100), (3, 200)] {
        let failed = world.run(world.control.work(&builder, at(now), LEASE)).unwrap_err();
        assert!(matches!(failed, control::Failed::Other(build::Error::Backend { source: Broke })), "attempt {n}");
        let state = world.run(world.control.job(id)).unwrap().unwrap().state;
        assert_eq!(state, if n < 3 { JobState::Running } else { JobState::Failed }, "after attempt {n}");
    }
    assert_eq!(
        world.run(world.control.work(&builder, at(1000), LEASE)).unwrap(),
        Worked::Idle,
        "a given-up request leaves the queue"
    );
}

#[test]
fn a_retry_waits_its_delay() {
    let world = World::new();
    let site = world.site("cstef");
    let (_, new) = request(&site, 10, None);
    world.run(world.control.request_build(new)).unwrap();
    let builder = Scripted::new(&world, [Step::Break, Step::Build("second try")]);
    world.run(world.control.work(&builder, at(11), LEASE)).unwrap_err();
    // released for 30 s from the failure
    assert_eq!(world.run(world.control.work(&builder, at(40), LEASE)).unwrap(), Worked::Idle);
    assert!(matches!(world.run(world.control.work(&builder, at(41), LEASE)).unwrap(), Worked::Built { .. }));
}

#[test]
fn a_duplicate_request_while_queued_is_dropped() {
    let world = World::new();
    let site = world.site("cstef");
    let (first, one) = request(&site, 10, Some("delivery-1"));
    let (_, two) = request(&site, 11, Some("delivery-1"));
    assert_eq!(world.run(world.control.request_build(one)).unwrap(), Requested::Queued(first));
    assert_eq!(world.run(world.control.request_build(two)).unwrap(), Requested::Duplicate);
    assert_eq!(world.run(world.control.jobs(site.id, 5)).unwrap().len(), 1, "a dropped request made no job");
}

#[test]
fn a_request_for_a_removed_site_is_dropped_and_said() {
    let world = World::new();
    let site = world.site("cstef");
    let (id, new) = request(&site, 10, None);
    world.run(world.control.request_build(new)).unwrap();
    world.run(world.control.remove_site(site.id, at(20))).unwrap();
    let builder = Scripted::new(&world, []);
    let worked = world.run(world.control.work(&builder, at(30), LEASE)).unwrap();
    assert_eq!(worked, Worked::Dropped(Dropped::NoJob { build: id }));
    assert!(builder.seen.borrow().is_empty(), "nothing was built for a removed site");
}

#[test]
fn a_message_that_names_no_build_is_dropped() {
    let world = World::new();
    world.site("cstef");
    let queue = world.control.ports().queue();
    let message = queue::Message { body: b"not a build id".to_vec(), dedupe: None };
    world.run(queue::Queue::send(queue, message, at(1))).unwrap();
    let builder = Scripted::new(&world, []);
    assert_eq!(world.run(world.control.work(&builder, at(2), LEASE)).unwrap(), Worked::Dropped(Dropped::Unnamed));
    assert_eq!(world.run(world.control.work(&builder, at(500), LEASE)).unwrap(), Worked::Idle);
}

#[test]
fn a_redelivered_request_does_not_reopen_an_ended_job() {
    let world = World::new();
    let site = world.site("cstef");
    let (id, new) = request(&site, 10, None);
    world.run(world.control.request_build(new)).unwrap();
    let builder = Scripted::new(&world, [Step::Build("hello"), Step::Refuse]);
    let worked = world.run(world.control.work(&builder, at(11), LEASE)).unwrap();
    let Worked::Built { recorded, .. } = worked else { panic!("{worked:?}") };
    // the queue delivers the same build again, as it may after a crash between settling and acking
    let queue = world.control.ports().queue();
    let message = queue::Message { body: id.to_string().into_bytes(), dedupe: None };
    world.run(queue::Queue::send(queue, message, at(12))).unwrap();
    assert_eq!(
        world.run(world.control.work(&builder, at(13), LEASE)).unwrap(),
        Worked::Dropped(Dropped::Settled { build: id })
    );
    let job = world.run(world.control.job(id)).unwrap().unwrap();
    assert_eq!((job.state, job.deploy), (JobState::Succeeded, Some(recorded.deploy)));
    assert_eq!(world.run(world.control.work(&builder, at(500), LEASE)).unwrap(), Worked::Idle);
}

/// A directory that cannot be written to, so a build's log cannot be stored.
struct Unwritable {
    inner: Directory,
}

impl Blob for Unwritable {
    type Error = Fault;
    type Read = Reading;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async; this stand-in does no waiting")]
    async fn put(
        &self,
        _: &Tenant,
        _: Hash,
        _: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, blob::Error<Fault>> {
        Err(blob::Error::Backend {
            source: Fault::Write { path: "injected".into(), source: std::io::Error::other("injected") },
        })
    }

    async fn get(&self, t: &Tenant, h: Hash) -> Result<Option<Reading>, blob::Error<Fault>> {
        self.inner.get(t, h).await
    }

    async fn stat(&self, t: &Tenant, h: Hash) -> Result<Option<Bytes>, blob::Error<Fault>> {
        self.inner.stat(t, h).await
    }

    async fn list(&self, t: &Tenant) -> Result<Vec<Listed>, blob::Error<Fault>> {
        self.inner.list(t).await
    }

    async fn delete(&self, t: &Tenant, h: Hash) -> Result<Deleted, blob::Error<Fault>> {
        self.inner.delete(t, h).await
    }
}

#[test]
fn a_failure_after_the_build_counts_as_an_attempt_and_ends_the_job() {
    let folder = tempfile::tempdir().unwrap();
    let control = World::over(Unwritable { inner: Directory::open(folder.path()).unwrap() });
    let account = control::Id::from_bytes([1; 16]);
    pollster::block_on(control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
    let site = pollster::block_on(control.add_site(NewSite {
        id: control::Id::from_bytes([2; 16]),
        account,
        handle: "cstef".parse().unwrap(),
        source: Source::Folder("/srv".into()),
        at: at(1),
    }))
    .unwrap();
    let (id, new) = request(&site, 10, None);
    pollster::block_on(control.request_build(new)).unwrap();
    let builder = Scripted::in_folder(folder.path(), [Step::Refuse, Step::Refuse, Step::Refuse]);
    for (n, now) in [(1, 11), (2, 100), (3, 200)] {
        let failed = pollster::block_on(control.work(&builder, at(now), LEASE)).unwrap_err();
        assert!(matches!(failed, control::Failed::Control(control::Error::Blob(_))), "attempt {n}: {failed:?}");
        let state = pollster::block_on(control.job(id)).unwrap().unwrap().state;
        assert_eq!(state, if n < 3 { JobState::Running } else { JobState::Failed }, "after attempt {n}");
    }
    assert_eq!(pollster::block_on(control.work(&builder, at(1000), LEASE)).unwrap(), Worked::Idle);
}
