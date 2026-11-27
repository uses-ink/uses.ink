//! Keys: enrolling is a device flow with a key pair as the secret, approval writes the key and
//! its event at once or not at all, a key is listed and revoked, and "last used" is a date.

use std::sync::Arc;

use control::keys::{
    Approval, Audit, Keys, Kind, Label, NewEnrolment, Poll, Refused, Right, Rights, Scope, Settings, State, UserCode,
    Why,
};
use control::signin::Ttl;
use control::{AccountKind, EnrolmentKind, Id, Ports};
use db_sqlite::Sqlite;
use sig::{KeyType, PublicKey, Seed, Signer};

use crate::world::{World, at};

fn keys(world: &World) -> Keys<Arc<Sqlite>> {
    Keys::new(world.control.ports().db().clone(), Settings::default())
}

fn signer(n: u8) -> Signer {
    Signer::from_seed(&Seed::new([n; 32]))
}

fn asking(world: &World, who: &Signer, purpose: Kind, site: Option<control::Id<control::SiteKind>>) -> NewEnrolment {
    NewEnrolment {
        id: world.id::<EnrolmentKind>(),
        code: UserCode::drawn([world.id::<AccountKind>().as_bytes()[0]; 16]),
        key_type: KeyType::Ed25519V1,
        public: who.public(),
        label: Label::parse("laptop").unwrap(),
        purpose,
        site,
        rights: Rights::NONE,
        lifetime: None,
        at: at(10),
    }
}

fn approval(new: &NewEnrolment, by: Id<AccountKind>, within: Rights) -> Approval {
    Approval { code: new.code.clone(), fingerprint: new.public.id(new.key_type).fingerprint(), by, within }
}

#[test]
fn a_person_approves_a_device_key_and_it_gets_the_least_that_works() {
    let world = World::new();
    let account = world.site("garden").account;
    let keys = keys(&world);
    let device = signer(1);
    let new = asking(&world, &device, Kind::Device, None);
    let enrolment = world.run(keys.enrol(new.clone())).unwrap();
    assert_eq!(enrolment.rights, Rights::DEVICE);
    assert_eq!(world.run(keys.waiting(at(11))).unwrap(), 1);
    let seen = world.run(keys.look(&new.code, at(11))).unwrap().unwrap();
    assert_eq!(seen.fingerprint, device.id().fingerprint());
    assert_eq!(seen.rights, Rights::DEVICE);
    assert_eq!(world.run(keys.poll(new.id, at(12))).unwrap().unwrap().1, Poll::Pending);

    let key = world.run(keys.approve(approval(&new, account, Rights::EVERY), at(20))).unwrap();
    assert_eq!(key.grant.scope, Scope::Account(account));
    assert!(!key.grant.rights.has(Right::Destroy) && key.grant.rights.has(Right::Deploy));
    assert_eq!(key.expires, at(20 + 180 * 86_400));
    assert_eq!(world.run(keys.key(device.id())).unwrap().as_ref(), Some(&key));
    assert_eq!(world.run(keys.list(account)).unwrap(), std::slice::from_ref(&key));
    assert_eq!(world.run(keys.poll(new.id, at(21))).unwrap().unwrap().1, Poll::Approved { expires: key.expires });
    let events = world.run(keys.events(account, 10)).unwrap();
    assert_eq!(events.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["key.approved"]);
    assert!(!events[0].body.contains("seed") && events[0].body.contains(&device.id().to_string()));
    // the code is spent: the same answer as for a code that never existed
    assert_eq!(
        world.run(keys.approve(approval(&new, account, Rights::EVERY), at(22))).unwrap_err().refusal(),
        Some(&Refused::NoEnrolment)
    );
    assert_eq!(world.run(keys.look(&new.code, at(22))).unwrap(), None);
    assert_eq!(world.run(keys.events(account, 10)).unwrap().len(), 1);
}

#[test]
fn an_unknown_an_expired_and_a_decided_code_are_the_same_refusal() {
    let world = World::new();
    let account = world.site("garden").account;
    let keys = keys(&world);
    let (first, second) =
        (asking(&world, &signer(1), Kind::Device, None), asking(&world, &signer(2), Kind::Device, None));
    world.run(keys.enrol(first.clone())).unwrap();
    world.run(keys.enrol(second.clone())).unwrap();
    world.run(keys.deny(&second.code, at(11))).unwrap();
    let late = at(10 + 11 * 60);
    let refusal = |code: &UserCode, when| {
        let new = NewEnrolment { code: code.clone(), ..first.clone() };
        world.run(keys.approve(approval(&new, account, Rights::EVERY), when)).unwrap_err().refusal().cloned()
    };
    let unknown: UserCode = "BBBB-BBBB".parse().unwrap();
    assert_eq!(refusal(&unknown, at(12)), Some(Refused::NoEnrolment));
    assert_eq!(refusal(&first.code, late), Some(Refused::NoEnrolment));
    assert_eq!(refusal(&second.code, at(12)), Some(Refused::NoEnrolment));
    assert_eq!(world.run(keys.poll(first.id, late)).unwrap().unwrap().1, Poll::Expired);
    assert_eq!(world.run(keys.poll(second.id, at(12))).unwrap().unwrap().1, Poll::Denied);
    assert!(world.run(keys.list(account)).unwrap().is_empty());
}

#[test]
fn what_the_person_did_not_compare_is_not_approved() {
    let world = World::new();
    let account = world.site("garden").account;
    let keys = keys(&world);
    let new = asking(&world, &signer(1), Kind::Device, None);
    world.run(keys.enrol(new.clone())).unwrap();
    let wrong = Approval { fingerprint: signer(2).id().fingerprint(), ..approval(&new, account, Rights::EVERY) };
    assert_eq!(world.run(keys.approve(wrong, at(12))).unwrap_err().refusal(), Some(&Refused::Fingerprint));
    let narrow = approval(&new, account, Rights::of(&[Right::Read]));
    assert_eq!(world.run(keys.approve(narrow, at(12))).unwrap_err().refusal(), Some(&Refused::Rights));
    assert!(world.run(keys.list(account)).unwrap().is_empty());
    assert_eq!(world.run(keys.waiting(at(12))).unwrap(), 1);
}

#[test]
fn a_deploy_key_is_one_site_of_the_approver_with_a_lifetime_under_the_ceiling() {
    let world = World::new();
    let site = world.site("garden");
    let other = world.site("wild");
    let keys = keys(&world);
    let ci = signer(3);
    assert_eq!(
        world.run(keys.enrol(asking(&world, &ci, Kind::Deploy, None))).unwrap_err().refusal(),
        Some(&Refused::Site)
    );
    let long =
        NewEnrolment { lifetime: Some(Ttl::secs(366 * 86_400)), ..asking(&world, &ci, Kind::Deploy, Some(site.id)) };
    assert!(matches!(world.run(keys.enrol(long)).unwrap_err().refusal(), Some(Refused::Lifetime { .. })));
    let strangers = asking(&world, &signer(4), Kind::Deploy, Some(other.id));
    world.run(keys.enrol(strangers.clone())).unwrap();
    assert_eq!(
        world.run(keys.approve(approval(&strangers, site.account, Rights::EVERY), at(12))).unwrap_err().refusal(),
        Some(&Refused::Site)
    );
    let new = asking(&world, &ci, Kind::Deploy, Some(site.id));
    world.run(keys.enrol(new.clone())).unwrap();
    let key = world.run(keys.approve(approval(&new, site.account, Rights::EVERY), at(20))).unwrap();
    assert_eq!(key.grant.scope, Scope::Site(site.id));
    assert_eq!(key.grant.rights, Rights::DEPLOY);
    assert_eq!(key.expires, at(20 + 90 * 86_400));
}

#[test]
fn a_key_is_revoked_once_with_its_event_and_last_use_is_a_date() {
    let world = World::new();
    let account = world.site("garden").account;
    let keys = keys(&world);
    let device = signer(1);
    let new = asking(&world, &device, Kind::Device, None);
    world.run(keys.enrol(new.clone())).unwrap();
    let key = world.run(keys.approve(approval(&new, account, Rights::EVERY), at(20))).unwrap();

    let today = Keys::<Arc<Sqlite>>::day(at(30));
    world.run(keys.used(key.id, today)).unwrap();
    world.run(keys.used(key.id, today)).unwrap();
    let listed = world.run(keys.list(account)).unwrap();
    assert_eq!(listed[0].last_used, Some(today));

    let print = key.id.fingerprint();
    let revoked = world.run(keys.revoke((account, &print), Why::Revoked, at(40))).unwrap();
    assert_eq!(revoked.state(at(41)), State::Revoked { at: at(40), why: Why::Revoked });
    assert_eq!(
        world.run(keys.revoke((account, &print), Why::Revoked, at(41))).unwrap_err().refusal(),
        Some(&Refused::NoKey)
    );
    let other = world.site("wild").account;
    assert_eq!(
        world.run(keys.revoke((other, &print), Why::Revoked, at(41))).unwrap_err().refusal(),
        Some(&Refused::NoKey)
    );
    let names: Vec<String> = world.run(keys.events(account, 10)).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["key.revoked", "key.approved"]);
    assert_eq!(
        world.run(keys.key(key.id)).unwrap().unwrap().state(at(41)),
        State::Revoked { at: at(40), why: Why::Revoked }
    );
}

#[test]
fn an_expired_key_says_so_and_events_anyone_can_repeat_are_written_once_an_hour() {
    let world = World::new();
    let account = world.site("garden").account;
    let keys = keys(&world);
    let new = NewEnrolment { lifetime: Some(Ttl::secs(100)), ..asking(&world, &signer(1), Kind::Device, None) };
    world.run(keys.enrol(new.clone())).unwrap();
    let key = world.run(keys.approve(approval(&new, account, Rights::EVERY), at(20))).unwrap();
    assert_eq!(key.state(at(119)), State::Live);
    assert_eq!(key.state(at(120)), State::Expired { at: at(120) });
    for second in [200, 201, 202] {
        world.run(keys.record(account, Audit::ReplayDetected { key: key.id }, at(second))).unwrap();
    }
    world.run(keys.record(account, Audit::ReplayDetected { key: key.id }, at(200 + 3_601))).unwrap();
    let replays =
        world.run(keys.events(account, 10)).unwrap().into_iter().filter(|e| e.name == "replay.detected").count();
    assert_eq!(replays, 2);
    let _: PublicKey = key.public;
}

#[test]
fn asking_again_replaces_the_ask_and_an_enrolled_key_cannot_ask() {
    let world = World::new();
    let account = world.site("garden").account;
    let keys = keys(&world);
    let device = signer(1);
    let first = asking(&world, &device, Kind::Device, None);
    world.run(keys.enrol(first.clone())).unwrap();
    let again = NewEnrolment { code: UserCode::drawn([9; 16]), ..asking(&world, &device, Kind::Device, None) };
    world.run(keys.enrol(again.clone())).unwrap();
    assert_eq!(world.run(keys.look(&first.code, at(11))).unwrap(), None);
    assert!(world.run(keys.look(&again.code, at(11))).unwrap().is_some());
    world.run(keys.approve(approval(&again, account, Rights::EVERY), at(20))).unwrap();
    let third = NewEnrolment { code: UserCode::drawn([8; 16]), ..asking(&world, &device, Kind::Device, None) };
    assert_eq!(world.run(keys.enrol(third)).unwrap_err().refusal(), Some(&Refused::KeyTaken));
}
