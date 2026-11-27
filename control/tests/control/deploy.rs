//! Deploys, the live pointer, rollback and resume.

use control::{NewDeploy, Published, Refusal, Resumed};
use model::{Hash, Manifest, MediaType, Route, Served};

use crate::world::{World, at};

fn new(site: &control::Site, manifest: Manifest, n: i64) -> NewDeploy {
    NewDeploy { site: site.id, manifest, build: None, at: at(n) }
}

#[test]
fn the_first_deploy_goes_live_and_the_second_remembers_the_first() {
    let world = World::new();
    let site = world.site("cstef");
    let one = world.stage(&site, &[("/", "one")]);
    let two = world.stage(&site, &[("/", "two"), ("/about", "about")]);
    let first = world.run(world.control.deploy(new(&site, one.clone(), 10))).unwrap();
    assert_eq!((first.deploy, first.published), (one.id(), Published::Live { previous: None }));
    let second = world.run(world.control.deploy(new(&site, two.clone(), 20))).unwrap();
    assert_eq!(second.published, Published::Live { previous: Some(one.id()) });
    let live = world.run(world.control.live(site.id)).unwrap().unwrap();
    assert_eq!((live.deploy, live.previous, live.paused), (two.id(), Some(one.id()), false));
    let listed = world.run(world.control.deploys(site.id, 10)).unwrap();
    let ids: Vec<_> = listed.iter().map(|d| (d.id, d.is_live, d.files)).collect();
    assert_eq!(ids, [(two.id(), true, 2), (one.id(), false, 1)]);
    assert_eq!(listed.first().unwrap().bytes, model::Bytes(8));
    assert_eq!(world.run(world.control.deploys(site.id, 1)).unwrap().len(), 1);
}

#[test]
fn the_same_build_twice_is_one_deploy() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest = world.stage(&site, &[("/", "same")]);
    world.run(world.control.deploy(new(&site, manifest.clone(), 10))).unwrap();
    let again = world.run(world.control.deploy(new(&site, manifest, 20))).unwrap();
    assert_eq!(again.published, Published::Unchanged);
    assert_eq!(world.run(world.control.deploys(site.id, 10)).unwrap().len(), 1);
    assert_eq!(world.count("SELECT COUNT(*) AS n FROM publication"), 1);
    assert_eq!(world.count("SELECT COUNT(*) AS n FROM event WHERE name = 'deploy.published'"), 1);
    assert_eq!(world.count("SELECT COUNT(*) AS n FROM event WHERE name = 'deploy.added'"), 1);
}

#[test]
fn refuses_a_manifest_whose_blob_is_missing_and_records_nothing() {
    let world = World::new();
    let site = world.site("cstef");
    let manifest =
        Manifest::new(vec![Served::new(Route::parse("/gone").unwrap(), Hash::of("never stored"), MediaType::Html, vec![])]).unwrap();
    let refused = world.run(world.control.deploy(new(&site, manifest, 10))).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::MissingBlob { .. })));
    assert_eq!(world.run(world.control.live(site.id)).unwrap(), None);
    assert!(world.run(world.control.deploys(site.id, 10)).unwrap().is_empty());
    assert_eq!(world.count("SELECT COUNT(*) AS n FROM event WHERE name LIKE 'deploy.%'"), 0);
}

#[test]
fn a_rollback_goes_back_pauses_publishing_and_resume_ends_the_pause() {
    let world = World::new();
    let site = world.site("cstef");
    let (one, two, three) = (
        world.stage(&site, &[("/", "one")]),
        world.stage(&site, &[("/", "two")]),
        world.stage(&site, &[("/", "three")]),
    );
    world.run(world.control.deploy(new(&site, one.clone(), 10))).unwrap();
    world.run(world.control.deploy(new(&site, two.clone(), 20))).unwrap();
    let back = world.run(world.control.rollback(site.id, one.id(), at(30))).unwrap();
    assert_eq!((back.deploy, back.previous, back.paused), (one.id(), Some(two.id()), true));
    // a build that finishes now is recorded and held, and does not undo the rollback
    let held = world.run(world.control.deploy(new(&site, three.clone(), 40))).unwrap();
    assert_eq!(held.published, Published::Held);
    assert_eq!(world.run(world.control.live(site.id)).unwrap().unwrap().deploy, one.id());
    assert_eq!(world.run(world.control.deploys(site.id, 10)).unwrap().len(), 3);
    assert_eq!(world.run(world.control.resume(site.id, at(50))).unwrap(), Resumed::Resumed);
    assert_eq!(world.run(world.control.resume(site.id, at(51))).unwrap(), Resumed::NotPaused);
    assert_eq!(world.run(world.control.live(site.id)).unwrap().unwrap().deploy, one.id(), "resume publishes nothing itself");
    let next = world.run(world.control.deploy(new(&site, three.clone(), 60))).unwrap();
    assert_eq!(next.published, Published::Live { previous: Some(one.id()) });
    // the history says what happened, in order
    let causes: Vec<String> = world.rows("SELECT cause FROM publication ORDER BY seq").iter().map(|r| r.get("cause").unwrap()).collect();
    assert_eq!(causes, ["publish", "publish", "rollback", "publish"]);
    let names: Vec<String> = world.rows("SELECT name FROM event ORDER BY seq").iter().map(|r| r.get("name").unwrap()).collect();
    assert!(names.contains(&"deploy.rolled-back".to_owned()) && names.contains(&"site.resumed".to_owned()));
    assert_eq!(names.iter().filter(|n| *n == "site.resumed").count(), 1, "the second resume changed nothing and logged nothing");
}

#[test]
fn rolling_back_twice_returns() {
    let world = World::new();
    let site = world.site("cstef");
    let (one, two) = (world.stage(&site, &[("/", "one")]), world.stage(&site, &[("/", "two")]));
    world.run(world.control.deploy(new(&site, one.clone(), 10))).unwrap();
    world.run(world.control.deploy(new(&site, two.clone(), 20))).unwrap();
    world.run(world.control.rollback(site.id, one.id(), at(30))).unwrap();
    let forward = world.run(world.control.rollback(site.id, two.id(), at(31))).unwrap();
    assert_eq!((forward.deploy, forward.previous), (two.id(), Some(one.id())));
}

#[test]
fn refuses_a_rollback_to_nothing_live_unknown_or_current() {
    let world = World::new();
    let site = world.site("cstef");
    let other = world.site("other");
    let one = world.stage(&site, &[("/", "one")]);
    let foreign = world.stage(&other, &[("/", "elsewhere")]);
    world.run(world.control.deploy(new(&other, foreign.clone(), 5))).unwrap();
    let refusal = |to, n| world.run(world.control.rollback(site.id, to, at(n))).unwrap_err();
    assert!(matches!(refusal(one.id(), 6).refusal(), Some(Refusal::NotLive { .. })));
    world.run(world.control.deploy(new(&site, one.clone(), 10))).unwrap();
    assert!(matches!(refusal(one.id(), 11).refusal(), Some(Refusal::AlreadyLive { .. })));
    // another site's deploy is not this site's to go back to
    assert!(matches!(refusal(foreign.id(), 12).refusal(), Some(Refusal::NoDeploy { .. })));
    assert_eq!(world.run(world.control.live(site.id)).unwrap().unwrap().deploy, one.id());
    assert!(!world.run(world.control.live(site.id)).unwrap().unwrap().paused, "a refused rollback must not pause");
}
