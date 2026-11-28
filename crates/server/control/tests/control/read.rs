//! An account's sites, the sites a poll looks at, and a build's log.

use build::Source;
use control::{NewSite, Refusal, SiteKind};

use crate::job::{LEASE, Scripted, Step, request};
use crate::world::{World, at};

#[test]
fn lists_the_sites_of_one_account_oldest_first() {
    let world = World::new();
    let first = world.site("first");
    let other = world.site("other");
    let second = NewSite {
        id: world.id::<SiteKind>(),
        account: first.account,
        handle: "second".parse().unwrap(),
        source: Source::Upload,
        at: at(5),
    };
    let second = world.run(world.control.add_site(second)).unwrap();
    assert_eq!(world.run(world.control.sites(first.account)).unwrap(), [first, second.clone()]);
    assert_eq!(world.run(world.control.sites(other.account)).unwrap(), [other]);
    assert_eq!(world.run(world.control.site(second.id)).unwrap().source, Source::Upload);
}

#[test]
fn watches_the_active_git_sites_only() {
    let world = World::new();
    let folder = world.site("folder");
    let git = |handle: &str, n: i64| NewSite {
        id: world.id::<SiteKind>(),
        account: folder.account,
        handle: handle.parse().unwrap(),
        source: Source::Git { url: "https://codeberg.org/cstef/blog".parse().unwrap(), revision: None },
        at: at(n),
    };
    let watched = world.run(world.control.add_site(git("watched", 5))).unwrap();
    let gone = world.run(world.control.add_site(git("gone", 6))).unwrap();
    world.run(world.control.remove_site(gone.id, at(7))).unwrap();
    assert_eq!(world.run(world.control.watched()).unwrap(), [watched]);
}

#[test]
fn reads_the_log_of_a_build_of_this_site_only() {
    let world = World::new();
    let site = world.site("cstef");
    let other = world.site("other");
    let (id, new) = request(&site, 10, None);
    world.run(world.control.request_build(new)).unwrap();
    assert_eq!(world.run(world.control.build_log(site.id, id)).unwrap(), None, "not built yet");
    let builder = Scripted::new(&world, [Step::Build("hello")]);
    world.run(world.control.work(&builder, at(11), LEASE)).unwrap();
    let log = world.run(world.control.build_log(site.id, id)).unwrap().unwrap();
    assert!(String::from_utf8(log).unwrap().starts_with("{\"event\":\"started\""));
    let error = world.run(world.control.build_log(other.id, id)).unwrap_err();
    assert!(matches!(error.refusal(), Some(Refusal::NoBuild { .. })));
}
