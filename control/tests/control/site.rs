//! Accounts, sites and handles.

use build::Source;
use control::table::{self, Site};
use control::{AccountKind, Handle, NewAccount, NewSite, Refusal, SiteKind, SiteState};

use crate::world::{World, at};

#[test]
fn adds_a_site_and_finds_it_by_id_and_handle() {
    let world = World::new();
    let site = world.site("cstef");
    assert_eq!(site.state, SiteState::Active);
    assert_eq!(world.run(world.control.site(site.id)).unwrap(), site);
    let handle: Handle = "cstef".parse().unwrap();
    assert_eq!(world.run(world.control.by_handle(&handle)).unwrap(), Some(site));
    let other: Handle = "nobody".parse().unwrap();
    assert_eq!(world.run(world.control.by_handle(&other)).unwrap(), None);
    assert_eq!(world.events("site.added"), 1);
}

#[test]
fn stores_a_git_source_as_the_user_wrote_it() {
    let world = World::new();
    let account = world.id::<AccountKind>();
    world.run(world.control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
    let source = Source::Git {
        url: model::ExternalUrl::parse("https://codeberg.org/cstef/blog").unwrap(),
        revision: Some("main".parse().unwrap()),
    };
    let new = NewSite {
        id: world.id::<SiteKind>(),
        account,
        handle: "diary".parse().unwrap(),
        source: source.clone(),
        at: at(1),
    };
    let id = new.id;
    world.run(world.control.add_site(new)).unwrap();
    assert_eq!(world.run(world.control.site(id)).unwrap().source, source);
}

#[test]
fn refuses_a_reserved_a_taken_and_an_unknown_account() {
    let world = World::new();
    let site = world.site("cstef");
    let new = |handle: &str, account| NewSite {
        id: world.id::<SiteKind>(),
        account,
        handle: handle.parse().unwrap(),
        source: Source::Folder("/srv".into()),
        at: at(2),
    };
    let refusal = |new| world.run(world.control.add_site(new)).unwrap_err();
    assert!(matches!(refusal(new("www", site.account)).refusal(), Some(Refusal::Reserved { .. })));
    assert!(matches!(refusal(new("cstef", site.account)).refusal(), Some(Refusal::Taken { .. })));
    let stranger = world.id::<AccountKind>();
    assert!(matches!(refusal(new("fresh", stranger)).refusal(), Some(Refusal::NoAccount { .. })));
    // a refused site leaves nothing behind
    assert_eq!(world.count(Site::Table), 1);
    assert_eq!(world.count(table::Source::Table), 1);
}

#[test]
fn refuses_an_account_that_exists() {
    let world = World::new();
    let id = world.id::<AccountKind>();
    world.run(world.control.add_account(NewAccount { id, at: at(0) })).unwrap();
    let again = world.run(world.control.add_account(NewAccount { id, at: at(1) })).unwrap_err();
    assert!(matches!(again.refusal(), Some(Refusal::Exists { .. })));
}

#[test]
fn says_a_site_is_missing() {
    let world = World::new();
    let missing = world.run(world.control.site(world.id::<SiteKind>())).unwrap_err();
    assert!(matches!(missing.refusal(), Some(Refusal::NoSite { .. })));
}
