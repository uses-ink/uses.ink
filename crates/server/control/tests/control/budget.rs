//! Budgets: what an account may store, build and have, and the sweep that keeps deploys to the
//! limit. Every refusal carries the value and the limit it read.

use blob::{Blob, Tenant};
use build::{BuildId, Source, Trigger};
use control::table::{Deploy, Job};
use control::{Limits, NewDeploy, NewJob, NewSite, Policy, Ports as _, Refusal, Spent};
use model::Bytes;

use crate::world::{World, at};

fn world(limits: Limits) -> World {
    World::new().under(Policy::DEFAULT.limited(limits))
}

fn deploy(site: &control::Site, manifest: model::Manifest, n: i64) -> NewDeploy {
    NewDeploy { site: site.id, manifest, build: None, at: at(n) }
}

fn build(site: &control::Site, id: u8, n: i64) -> NewJob {
    NewJob { id: BuildId::from_bytes([id; 16]), site: site.id, trigger: Trigger::Push, dedupe: None, at: at(n) }
}

fn blobs(world: &World, site: &control::Site) -> usize {
    let tenant = Tenant::try_from(site.id).unwrap();
    world.run(world.control.ports().blob().list(&tenant)).unwrap().len()
}

/// A second site of the account `site` belongs to.
fn sibling(world: &World, site: &control::Site, handle: &str) -> control::Site {
    world
        .run(world.control.add_site(NewSite {
            id: world.id(),
            account: site.account,
            handle: handle.parse().unwrap(),
            source: Source::Upload,
            at: at(2),
        }))
        .unwrap()
}

#[test]
fn a_deploy_over_the_storage_budget_is_refused_whole() {
    let world = world(Limits { stored: Bytes(1500), ..Limits::DEFAULT });
    let site = world.site("cstef");
    let first = world.stage(&site, &[("/", "a".repeat(1000).as_str())]);
    world.run(world.control.deploy(deploy(&site, first.clone(), 10))).unwrap();
    let before = blobs(&world, &site);
    let second = world.stage(&site, &[("/", "b".repeat(1000).as_str())]);
    let refused = world.run(world.control.deploy(deploy(&site, second, 20))).unwrap_err();
    let Some(Refusal::Over { spent: Spent::Stored, value, limit: 1500 }) = refused.refusal() else {
        panic!("a storage refusal, got {refused:?}")
    };
    assert!(*value > 2000, "the value is what the account would store: {value}");
    assert_eq!(refused.refusal().map(Refusal::code), Some("budget.stored.over"));
    // nothing of the refused deploy is recorded, served or left in the store
    assert_eq!(world.count(Deploy::Table), 1);
    assert_eq!(world.run(world.control.live(site.id)).unwrap().unwrap().deploy, first.id());
    assert_eq!(blobs(&world, &site), before);
    assert!(world.run(world.control.stored(site.account)).unwrap().count() < 1500);
}

#[test]
fn a_deploy_that_fits_exactly_is_taken() {
    let world = world(Limits { stored: Bytes(1000), ..Limits::DEFAULT });
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "a".repeat(1000).as_str())]);
    // the manifest is stored with the deploy, so the files alone fill the budget
    let taken = world.run(world.control.deploy(deploy(&site, manifest, 10)));
    assert!(taken.is_ok(), "{taken:?}");
}

#[test]
fn storage_is_counted_over_every_site_of_the_account() {
    let world = world(Limits { stored: Bytes(1500), ..Limits::DEFAULT });
    let one = world.site("cstef");
    let two = sibling(&world, &one, "diary");
    let first = world.stage(&one, &[("/", "a".repeat(1000).as_str())]);
    world.run(world.control.deploy(deploy(&one, first, 10))).unwrap();
    let second = world.stage(&two, &[("/", "b".repeat(1000).as_str())]);
    let refused = world.run(world.control.deploy(deploy(&two, second, 20))).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::Over { spent: Spent::Stored, .. })), "{refused:?}");
    // another account has its own budget
    let other = world.site("elsewhere");
    let third = world.stage(&other, &[("/", "c".repeat(1000).as_str())]);
    world.run(world.control.deploy(deploy(&other, third, 30))).unwrap();
}

#[test]
fn older_deploys_are_swept_to_make_room_and_to_keep_the_limit() {
    let world = world(Limits { stored: Bytes(2900), deploys: 2, ..Limits::DEFAULT });
    let site = world.site("cstef");
    let shared = "s".repeat(500);
    let mut manifests = Vec::new();
    for (n, letter) in [(10, "a"), (20, "b"), (30, "c")] {
        let manifest = world.stage(&site, &[("/", letter.repeat(1000).as_str()), ("/shared", shared.as_str())]);
        world.run(world.control.deploy(deploy(&site, manifest.clone(), n))).unwrap();
        manifests.push(manifest);
    }
    // the third would not fit beside two; the first was swept, and the two newest are left
    let kept = world.run(world.control.deploys(site.id, 10)).unwrap();
    let ids: Vec<_> = kept.iter().map(|d| d.id).collect();
    assert_eq!(ids, [manifests[2].id(), manifests[1].id()]);
    assert_eq!(world.count(Deploy::Table), 2);
    // the first deploy's own file and manifest are gone; the shared file and the kept ones stay
    let tenant = Tenant::try_from(site.id).unwrap();
    let listed = world.run(world.control.ports().blob().list(&tenant)).unwrap();
    let held: Vec<_> = listed.iter().map(|listed| listed.hash).collect();
    assert!(!held.contains(&manifests[0].id().hash()));
    assert!(held.contains(&model::Hash::of("s".repeat(500))));
    assert!(held.contains(&manifests[2].id().hash()) && held.contains(&manifests[1].id().hash()));
    assert_eq!(held.len(), 5, "two manifests, two own files, the shared file");
    // the one replaced is still a rollback target
    world.run(world.control.rollback(site.id, manifests[1].id(), at(40))).unwrap();
}

#[test]
fn sweeping_below_the_limit_changes_nothing() {
    let world = world(Limits::DEFAULT);
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "one")]);
    world.run(world.control.deploy(deploy(&site, manifest, 10))).unwrap();
    assert_eq!(world.run(world.control.sweep(site.id)).unwrap(), control::Swept::default());
}

#[test]
fn builds_a_day_are_limited_and_the_day_passes() {
    let world = world(Limits { builds: 2, ..Limits::DEFAULT });
    let site = world.site("cstef");
    world.run(world.control.request_build(build(&site, 1, 10))).unwrap();
    world.run(world.control.request_build(build(&site, 2, 11))).unwrap();
    let refused = world.run(world.control.request_build(build(&site, 3, 12))).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::Over { spent: Spent::Builds, value: 2, limit: 2 }));
    assert_eq!(
        refused.refusal().map(Refusal::fields),
        Some(vec![("count", "2".to_owned()), ("limit", "2".to_owned())])
    );
    assert_eq!(world.count(Job::Table), 2, "a refused build queues nothing");
    // 24 hours after the first two, the account has built nothing in the last day
    world.run(world.control.request_build(build(&site, 4, 10 + 86_401))).unwrap();
}

#[test]
fn sites_per_account_are_limited() {
    let world = world(Limits { sites: 1, ..Limits::DEFAULT });
    let site = world.site("cstef");
    let refused = world
        .run(world.control.add_site(NewSite {
            id: world.id(),
            account: site.account,
            handle: "diary".parse().unwrap(),
            source: Source::Upload,
            at: at(2),
        }))
        .unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refusal::Over { spent: Spent::Sites, value: 1, limit: 1 }));
    assert_eq!(refused.refusal().map(Refusal::code), Some("budget.sites.over"));
    world.site("elsewhere");
}
