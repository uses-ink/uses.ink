//! The grace of a blob nothing names (decision 0150): a build that skipped uploading a blob
//! because the store held it can still record the deploy that names it.

use std::time::Duration;

use blob::{Blob, Tenant};
use control::{Limits, NewDeploy, Policy, Ports as _, Refusal};
use model::Hash;

use crate::world::{World, at};

const GRACE: Duration = Duration::from_mins(10);

/// A world that keeps one deploy and gives an unnamed blob `grace`.
fn world(grace: Duration) -> World {
    World::new().under(Policy::DEFAULT.limited(Limits { deploys: 1, ..Limits::DEFAULT }).graced(grace))
}

fn deploy(site: &control::Site, manifest: model::Manifest, n: i64) -> NewDeploy {
    NewDeploy { site: site.id, manifest, build: None, at: at(n) }
}

fn held(world: &World, site: &control::Site, text: &str) -> bool {
    let tenant = Tenant::try_from(site.id).unwrap();
    world.run(world.control.ports().blob().exists(&tenant, Hash::of(text))).unwrap()
}

/// The first deploy names `shared`; two more drop it, so the sweep queues it at 40.
fn dropped(world: &World, site: &control::Site) {
    let first = world.stage(site, &[("/", "shared")]);
    world.run(world.control.deploy(deploy(site, first, 10))).unwrap();
    for (n, text) in [(20, "second"), (30, "third")] {
        let manifest = world.stage(site, &[("/", text)]);
        world.run(world.control.deploy(deploy(site, manifest, n))).unwrap();
    }
}

#[test]
fn a_build_that_skipped_its_upload_records_its_deploy_while_the_blob_waits() {
    let world = world(GRACE);
    let site = world.site("cstef");
    let first = world.stage(&site, &[("/", "shared")]);
    world.run(world.control.deploy(deploy(&site, first, 10))).unwrap();
    // the build uploads again: the store holds `shared`, so the put is skipped and no row sees it
    let build = world.stage(&site, &[("/", "shared"), ("/more", "own")]);
    for (n, text) in [(20, "second"), (30, "third")] {
        let manifest = world.stage(&site, &[("/", text)]);
        world.run(world.control.deploy(deploy(&site, manifest, n))).unwrap();
    }
    assert!(held(&world, &site, "shared"), "the dropped deploy's blob waits out its grace");
    let recorded = world.run(world.control.deploy(deploy(&site, build, 40)));
    recorded.unwrap();
    world.run(world.control.reap(at(40 + 601))).unwrap();
    assert!(held(&world, &site, "shared"), "the recorded deploy names the blob again");
}

#[test]
fn a_blob_nothing_names_stays_through_its_grace_and_goes_after_it() {
    let world = world(GRACE);
    let site = world.site("cstef");
    dropped(&world, &site);
    assert!(held(&world, &site, "shared"));
    let early = world.run(world.control.reap(at(30 + 599))).unwrap();
    assert_eq!(early.blobs, 0);
    assert!(held(&world, &site, "shared"));
    let due = world.run(world.control.reap(at(30 + 600))).unwrap();
    assert_eq!(due.blobs, 2, "the file and the dropped deploy's manifest");
    assert!(!held(&world, &site, "shared"));
    assert_eq!(world.count(control::table::Doomed::Table), 0, "the queue is empty once it is reaped");
}

#[test]
fn the_account_is_not_charged_for_a_blob_that_waits() {
    let world = world(GRACE);
    let site = world.site("cstef");
    dropped(&world, &site);
    assert!(held(&world, &site, "shared"));
    let tenant = Tenant::try_from(site.id).unwrap();
    let stored: u64 =
        world.run(world.control.ports().blob().list(&tenant)).unwrap().iter().map(|l| l.size.count()).sum();
    assert!(world.run(world.control.stored(site.account)).unwrap().count() < stored);
}

#[test]
fn with_no_grace_a_skipped_upload_is_lost_and_the_deploy_says_which_blob() {
    let world = world(Duration::ZERO);
    let site = world.site("cstef");
    let first = world.stage(&site, &[("/", "shared")]);
    world.run(world.control.deploy(deploy(&site, first, 10))).unwrap();
    let build = world.stage(&site, &[("/", "shared"), ("/more", "own")]);
    for (n, text) in [(20, "second"), (30, "third")] {
        let manifest = world.stage(&site, &[("/", text)]);
        world.run(world.control.deploy(deploy(&site, manifest, n))).unwrap();
    }
    assert!(!held(&world, &site, "shared"));
    let refused = world.run(world.control.deploy(deploy(&site, build, 40))).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::MissingBlob { .. })));
}
