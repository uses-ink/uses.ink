//! The `git` capability through the public API: local `file://` repositories built here, object by
//! object, fetched into a cache folder under the target directory. No network is touched; gix's
//! file transport runs the `git upload-pack` of the machine, so `git` must be on the path.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_native as host_native;

#[path = "support/origin.rs"]
mod origin;

use host_native::{Cloning, Failed, Fetch, FetchLimits, Memory, Repository};
use model::git::{Auth, Budget, Depth, Git, Network, Refused, Request, Scheme};
use model::{Bytes, Diagnostic, Known, Provenance, Version};
use fixture::origin::Origin;
use origin::Fetching as _;

fn code<E: std::error::Error>(refused: &Refused<E>) -> String {
    Diagnostic::from(refused).code().to_string()
}

#[test]
fn the_network_is_off_until_the_operator_says_otherwise() {
    let origin = Origin::named("off");
    let refused = Fetch::new(&origin.cache).run(&origin.request(Depth::Full)).unwrap_err();
    assert!(matches!(refused, Refused::Network { scheme: Scheme::File, policy: Network::Off }), "{refused:?}");
    assert!(!origin.cache.exists(), "a refused fetch touches no disk");
    assert_eq!(code(&refused), "source.git.network");
}

#[test]
fn a_local_setting_refuses_https_and_an_open_one_tries_it() {
    let origin = Origin::named("https");
    let https = Request::new("https://127.0.0.1:1/site.git".parse().unwrap());
    let local = origin.fetch().run(&https).unwrap_err();
    assert!(matches!(local, Refused::Network { scheme: Scheme::Https, policy: Network::Local }), "{local:?}");
    let open = Fetch { network: Network::On, ..origin.fetch() }.run(&https).unwrap_err();
    assert!(matches!(open, Refused::Failed(Failed::Fetch { .. })), "{open:?}");
}

#[test]
fn a_credential_the_builder_does_not_hold_is_a_typed_refusal_before_any_fetch() {
    let origin = Origin::named("auth");
    let grant = "github".parse().unwrap();
    let mut request = origin.request(Depth::Full);
    request.grant = Some(grant);
    let refused = origin.fetch().run(&request).unwrap_err();
    assert!(matches!(refused, Refused::Auth { outcome: Auth::Absent, .. }), "{refused:?}");
    assert_eq!(code(&refused), "source.git.auth");

    let mut held = origin.fetch();
    held.grants.hold("github".parse().unwrap(), "x-access-token", "s3cret");
    assert!(!format!("{held:?}").contains("s3cret"), "a secret redacts itself");
    held.run(&request).unwrap();
}

#[test]
fn a_full_fetch_reads_the_snapshot_the_repository_itself_reads() {
    let origin = Origin::named("full");
    let fetch = origin.fetch();
    let cloned = fetch.run(&origin.request(Depth::Full)).unwrap();
    assert!(!cloned.shallow);
    assert!(cloned.objects >= 9, "three commits, trees and blobs: {}", cloned.objects);
    assert!(cloned.bytes > Bytes(0));

    let mut store = Memory::default();
    let mut cloning = Cloning::new(&fetch, &mut store);
    let snapshot = cloning.snapshot(&cloned).unwrap();
    let direct = Repository::new(&origin.root).read(&mut Memory::default()).unwrap().snapshot;
    assert_eq!(snapshot, direct);
    assert_eq!(snapshot.version(), &Version::GitCommit(cloned.commit.clone()));
    let facts = snapshot.facts_of(&"a.md".parse().unwrap()).unwrap();
    assert!(matches!(&facts.created, Known::Exact { from: Provenance::Commit { .. }, .. }));
    assert_eq!(store.len(), 2, "every file's bytes are in the store the caller gave");
}

#[test]
fn a_shallow_fetch_lacks_history_and_a_full_one_deepens_it() {
    let origin = Origin::named("deepen");
    let fetch = origin.fetch();
    let one = std::num::NonZeroU32::new(1).unwrap();
    let shallow = fetch.run(&origin.request(Depth::Commits(one))).unwrap();
    assert!(shallow.shallow);
    let bounded = Cloning::new(&fetch, &mut Memory::default()).snapshot(&shallow).unwrap();
    assert!(
        bounded.observed().lacks.iter().any(|lack| matches!(lack.gap, model::Gap::Shallow { .. })),
        "{:?}",
        bounded.observed()
    );

    let full = fetch.run(&origin.request(Depth::Full)).unwrap();
    assert!(!full.shallow, "a full request undoes the boundary of the clone it finds");
    assert_eq!(full.clone, shallow.clone, "the same repository is the same clone");
    let deepened = Cloning::new(&fetch, &mut Memory::default()).snapshot(&full).unwrap();
    let direct = Repository::new(&origin.root).read(&mut Memory::default()).unwrap().snapshot;
    assert_eq!(deepened, direct);
}

#[test]
fn a_clone_past_a_budget_is_refused_with_its_numbers_and_removed() {
    let origin = Origin::named("budget");
    let bytes = Fetch { limits: FetchLimits { bytes: Bytes(1), ..FetchLimits::default() }, ..origin.fetch() };
    let refused = bytes.run(&origin.request(Depth::Full)).unwrap_err();
    assert!(matches!(refused, Refused::Budget(Budget::Bytes { limit: Bytes(1), .. })), "{refused:?}");
    assert_eq!(code(&refused), "source.budget.bytes");
    assert!(std::fs::read_dir(origin.cache.join("git")).unwrap().next().is_none(), "the clone is removed");

    let objects = Fetch { limits: FetchLimits { objects: 2, ..FetchLimits::default() }, ..origin.fetch() };
    let refused = objects.run(&origin.request(Depth::Full)).unwrap_err();
    assert!(matches!(refused, Refused::Budget(Budget::Objects { limit: 2, .. })), "{refused:?}");
    assert_eq!(code(&refused), "source.budget.objects");
}

#[test]
fn a_remote_that_is_not_there_is_a_failure_that_keeps_its_cause() {
    let origin = Origin::named("missing");
    let gone = Request::new(format!("file://{}/nothing", origin.root.display()).parse().unwrap());
    let refused = origin.fetch().run(&gone).unwrap_err();
    assert!(matches!(refused, Refused::Failed(Failed::Fetch { .. })), "{refused:?}");
    assert!(std::error::Error::source(&refused).is_some());
    assert_eq!(code(&refused), "source.repository.unreadable");
}

#[test]
fn a_named_branch_is_the_revision_read() {
    let origin = Origin::named("branch");
    let mut main = origin.request(Depth::Full);
    main.revision = Some("main".parse().unwrap());
    let cloned = origin.fetch().run(&main).unwrap();
    let head = Repository::new(&origin.root).read(&mut Memory::default()).unwrap().snapshot;
    assert_eq!(head.version(), &Version::GitCommit(cloned.commit));
}
