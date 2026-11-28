//! Handles: the brand list, the hold after a site leaves, the verified owner and renaming.

use std::time::Duration;

use build::Source;
use control::table::{Hold, Tombstone};
use control::{AccountKind, Error, HoldReason, Id, NewSite, Ports as _, Refusal, Reservation, Site, SiteKind};
use db::Db as _;
use jiff::Timestamp;
use sea_query::{Expr, Query};

use crate::world::{World, at};

/// The hold the tests use: a thousand seconds.
const HOLD: Duration = Duration::from_secs(1000);

fn world() -> World {
    World::new().under(World::BARE.holding(HOLD))
}

/// Claims `handle` for a new account's site at `time`, with an upload source.
fn claim(world: &World, handle: &str, time: Timestamp) -> Result<Site, Error<crate::world::Wired>> {
    let account: Id<AccountKind> = world.id();
    world.run(world.control.add_account(control::NewAccount { id: account, at: time })).unwrap();
    world.run(world.control.add_site(NewSite {
        id: world.id::<SiteKind>(),
        account,
        handle: handle.parse().unwrap(),
        source: Source::Upload,
        at: time,
    }))
}

#[test]
fn a_reserved_name_is_refused_with_the_list_it_is_on() {
    let world = world();
    for (name, list) in [("www", Reservation::Infrastructure), ("github", Reservation::Brand)] {
        let refused = claim(&world, name, at(1)).unwrap_err();
        assert_eq!(refused.refusal(), Some(&Refusal::Reserved { handle: name.parse().unwrap(), reservation: list }));
    }
    assert_eq!(world.count(control::table::Site::Table), 0);
}

#[test]
fn the_first_claim_wins_whatever_the_source() {
    let world = world();
    world.site("diary");
    let again = claim(&world, "diary", at(2)).unwrap_err();
    assert!(matches!(again.refusal(), Some(Refusal::Taken { .. })));
}

#[test]
fn a_removed_handle_is_not_claimable_for_the_whole_hold() {
    let world = world();
    let site = world.site("cstef");
    world.run(world.control.remove_site(site.id, at(100))).unwrap();
    let until = at(100 + 1000);
    // every second of the hold refuses with the end of the hold, and none takes the handle
    for second in (101..1100).step_by(111) {
        let refused = claim(&world, "cstef", at(second)).unwrap_err();
        let expected = Refusal::Held { handle: "cstef".parse().unwrap(), why: HoldReason::Removed, until: Some(until) };
        assert_eq!(refused.refusal(), Some(&expected), "at {second}");
    }
    assert_eq!(world.count(control::table::Site::Table), 0);
    // the hold ends exactly at `until`, and claiming releases it and the tombstone
    let claimed = claim(&world, "cstef", until).unwrap();
    assert_eq!(claimed.handle.as_str(), "cstef");
    assert_eq!((world.count(Hold::Table), world.count(Tombstone::Table)), (0, 0));
}

#[test]
fn a_verified_handle_is_never_reclaimed() {
    let world = world();
    let site = world.site("cstef");
    let hold = world.run(world.control.verify(site.id, at(50))).unwrap();
    assert_eq!((hold.why, hold.until), (HoldReason::Verified, None));
    // verifying again changes nothing and logs nothing
    world.run(world.control.verify(site.id, at(60))).unwrap();
    assert_eq!(world.events("handle.verified"), 1);
    world.run(world.control.remove_site(site.id, at(100))).unwrap();
    for second in [101, 1100, 1101, 100_000, 10_000_000_000] {
        let refused = claim(&world, "cstef", at(second)).unwrap_err();
        let expected = Refusal::Held { handle: "cstef".parse().unwrap(), why: HoldReason::Verified, until: None };
        assert_eq!(refused.refusal(), Some(&expected), "at {second}");
        assert_eq!(refused.refusal().map(Refusal::code), Some("handle.name.kept"));
    }
}

#[test]
fn a_site_that_is_alive_answers_taken_even_when_verified() {
    let world = world();
    let site = world.site("cstef");
    world.run(world.control.verify(site.id, at(50))).unwrap();
    let refused = claim(&world, "cstef", at(60)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::Taken { .. })));
}

#[test]
fn a_tombstone_from_before_holds_is_held_from_its_day() {
    let world = world();
    let insert = Query::insert()
        .into_table(Tombstone::Table)
        .columns([Tombstone::Handle, Tombstone::SiteId, Tombstone::RemovedAt])
        .values([Expr::val("old-name"), Expr::val("0".repeat(32)), Expr::val(at(0).as_millisecond())])
        .unwrap()
        .to_owned();
    world.run(world.control.ports().db().run(insert.into())).unwrap();
    let refused = claim(&world, "old-name", at(10)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::Held { why: HoldReason::Removed, until: Some(_), .. })));
    claim(&world, "old-name", at(1000)).unwrap();
}

#[test]
fn a_rename_holds_the_old_handle() {
    let world = world();
    let site = world.site("cstef");
    let renamed = world.run(world.control.rename_site(site.id, "diary".parse().unwrap(), at(10))).unwrap();
    assert_eq!(renamed.handle.as_str(), "diary");
    assert_eq!(world.events("site.renamed"), 1);
    let refused = claim(&world, "cstef", at(11)).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::Held { why: HoldReason::Renamed, .. })));
    // the owner may take it back once it is theirs to claim again: after the hold
    claim(&world, "cstef", at(10 + 1000)).unwrap();
}

#[test]
fn a_rename_is_refused_like_a_claim() {
    let world = world();
    let site = world.site("cstef");
    world.site("diary");
    for (name, code) in [
        ("www", "handle.name.reserved"),
        ("github", "handle.name.brand"),
        ("diary", "handle.name.taken"),
        ("cstef", "handle.name.taken"),
    ] {
        let refused = world.run(world.control.rename_site(site.id, name.parse().unwrap(), at(10))).unwrap_err();
        assert_eq!(refused.refusal().map(Refusal::code), Some(code), "{name}");
    }
    assert_eq!(world.run(world.control.site(site.id)).unwrap().handle.as_str(), "cstef");
}

#[test]
fn every_refusal_carries_its_data_and_a_code() {
    let world = world();
    let site = world.site("cstef");
    world.run(world.control.remove_site(site.id, at(100))).unwrap();
    let refused = claim(&world, "cstef", at(101)).unwrap_err();
    let refusal = refused.refusal().unwrap();
    assert_eq!(refusal.code(), "handle.name.held");
    let fields = refusal.fields();
    assert_eq!(fields[0], ("handle", "cstef".to_owned()));
    assert_eq!(fields[1].0, "until");
}
