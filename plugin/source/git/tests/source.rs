//! The git source's rule over the host's own `git` capability, and over a stand-in that lies.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

#[path = "../../../../host/native/tests/support/origin.rs"]
mod origin;

use host_native::{Cloning, Memory, Repository};
use model::git::{Cloned, Git, History, Network, Refused, Request, Want};
use model::{Bytes, Gap, Hash, Observed, Oid, Snapshot, Version};
use origin::Origin;
use uses_ink_source_git::{Source, Unread};

fn want(origin: &Origin, history: History) -> Want {
    let mut want = Want::new(origin.locator());
    want.history = history;
    want
}

#[test]
fn the_source_reads_the_snapshot_the_repository_itself_reads() {
    let origin = Origin::new("core-full");
    let (fetch, mut store) = (origin.fetch(), Memory::default());
    let snapshot = Source(Cloning::new(&fetch, &mut store)).read(&want(&origin, History::Wanted)).unwrap();
    let direct = Repository::new(&origin.root).read(&mut Memory::default()).unwrap().snapshot;
    assert_eq!(snapshot, direct);
    assert_eq!(store.len(), 2);
}

#[test]
fn a_site_that_needs_no_history_fetches_one_commit_and_its_dates_are_bounds() {
    let origin = Origin::new("core-snapshot");
    let (fetch, mut store) = (origin.fetch(), Memory::default());
    let snapshot = Source(Cloning::new(&fetch, &mut store)).read(&want(&origin, History::Unneeded)).unwrap();
    assert!(snapshot.observed().lacks.iter().any(|lack| matches!(lack.gap, Gap::Shallow { .. })));
    assert!(snapshot.facts().iter().all(|facts| facts.updated.exact().is_none()), "dates beyond the window are bounds");
}

#[test]
fn what_the_host_refuses_comes_back_typed() {
    let origin = Origin::new("core-off");
    let (mut fetch, mut store) = (origin.fetch(), Memory::default());
    fetch.network = Network::Off;
    let refused = Source(Cloning::new(&fetch, &mut store)).read(&want(&origin, History::Wanted)).unwrap_err();
    assert!(matches!(refused, Unread::Refused(Refused::Network { .. })), "{refused:?}");
}

/// A host that fetches one commit and then hands over a snapshot of another.
struct Liar;

impl Git for Liar {
    type Error = std::convert::Infallible;

    fn fetch(&mut self, _request: &Request) -> Result<Cloned, Refused<Self::Error>> {
        Ok(Cloned {
            clone: Hash::of(b"x"),
            commit: Oid::parse("1".repeat(40)).unwrap(),
            shallow: false,
            objects: 1,
            bytes: Bytes(1),
        })
    }

    fn snapshot(&mut self, _cloned: &Cloned) -> Result<Snapshot, Refused<Self::Error>> {
        let other = Version::GitCommit(Oid::parse("2".repeat(40)).unwrap());
        Ok(Snapshot::new(other, Vec::new(), Vec::new(), Observed::default()).unwrap())
    }
}

#[test]
fn a_snapshot_of_another_commit_than_the_one_fetched_is_refused() {
    let origin = Origin::new("core-liar");
    let refused = Source(Liar).read(&want(&origin, History::Wanted)).unwrap_err();
    assert!(matches!(refused, Unread::Version { .. }), "{refused:?}");
}
