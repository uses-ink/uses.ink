//! Signing in: links are single use and expire, only digests are stored, sessions end, and the
//! first-run link is made once.

use std::fmt::Write as _;

use control::signin::{Drawn, Expiry, Presented, Proof, Purpose, Refused, Requested, Settings, SignIn, Signup};
use control::{AccountKind, Id, Ports, table};
use mail::Address;
use sea_query::{Query, SelectStatement};

use crate::world::{World, at};

struct Door {
    world: World,
    next: std::cell::Cell<u8>,
}

impl Door {
    fn new() -> Self {
        Self { world: World::new(), next: std::cell::Cell::new(1) }
    }

    fn with(&self, settings: Settings) -> SignIn<std::sync::Arc<db_sqlite::Sqlite>> {
        SignIn::new(self.world.control.ports().db().clone(), settings)
    }

    fn drawn(&self) -> Drawn {
        let n = self.next.get();
        self.next.set(n + 1);
        Drawn::new([
            [n; 16],
            [n.wrapping_add(100); 16],
            [n.wrapping_add(50); 16],
            [n.wrapping_add(150); 16],
            [n.wrapping_add(200); 16],
        ])
    }

    fn address(text: &str) -> Address {
        text.parse().unwrap()
    }

    /// Asks for a link and returns its secret.
    fn link(&self, entry: &SignIn<std::sync::Arc<db_sqlite::Sqlite>>, who: &str, now: i64) -> Option<Proof> {
        match self.world.run(entry.request(&Self::address(who), self.drawn(), at(now))).unwrap() {
            Requested::Issued(issued) => Some(issued.proof),
            _ => None,
        }
    }

    fn rows(&self, select: &mut SelectStatement) -> Vec<db::Row> {
        self.world.rows(select)
    }
}

fn settings(signup: Signup) -> Settings {
    Settings { signup, ..Settings::default() }
}

#[test]
fn a_link_signs_in_once_and_makes_the_first_account() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let proof = door.link(&entry, "Me@Example.org", 0).unwrap();
    let pending = door.world.run(entry.peek(&proof, at(1))).unwrap().unwrap();
    assert_eq!(pending.purpose, Purpose::Login);
    // looking does not spend it
    assert!(door.world.run(entry.peek(&proof, at(2))).unwrap().is_some());
    let entered =
        door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(3))).unwrap();
    assert!(entered.created);
    let session = door.world.run(entry.resolve(&entered.secret, at(4))).unwrap().unwrap();
    assert_eq!(session.account, entered.account);
    let factors = door.world.run(entry.factors(entered.account)).unwrap();
    assert_eq!(factors.len(), 1);
    // used: the same link is nothing now, and so is a peek at it
    let again =
        door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(5))).unwrap_err();
    assert_eq!(again.refusal(), Some(&Refused::Unusable));
    assert!(door.world.run(entry.peek(&proof, at(5))).unwrap().is_none());
}

#[test]
fn a_known_address_signs_in_to_its_account_and_makes_no_second() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let first = door.link(&entry, "me@example.org", 0).unwrap();
    let entered =
        door.world.run(entry.redeem(Presented { proof: &first, address: None }, door.drawn(), at(1))).unwrap();
    // the first account exists, so the default policy makes no other, but this address signs in
    let second = door.link(&entry, "ME@example.org", 10).unwrap();
    let again =
        door.world.run(entry.redeem(Presented { proof: &second, address: None }, door.drawn(), at(11))).unwrap();
    assert!(!again.created);
    assert_eq!(again.account, entered.account);
    assert_eq!(door.world.count(table::Account::Table), 1);
    assert!(door.link(&entry, "other@example.org", 20).is_none());
}

#[test]
fn a_link_ends_and_a_changed_one_is_nothing() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let proof = door.link(&entry, "me@example.org", 0).unwrap();
    let ttl = Settings::default().login.duration().as_secs().try_into().unwrap();
    let late =
        door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(ttl))).unwrap_err();
    assert_eq!(late.refusal(), Some(&Refused::Unusable));
    let proof = door.link(&entry, "me@example.org", 100).unwrap();
    let mut text = proof.reveal().to_owned();
    let flipped = if text.starts_with('a') { "b" } else { "a" };
    text.replace_range(0..1, flipped);
    let tampered: Proof = text.parse().unwrap();
    let error =
        door.world.run(entry.redeem(Presented { proof: &tampered, address: None }, door.drawn(), at(101))).unwrap_err();
    assert_eq!(error.refusal(), Some(&Refused::Unusable));
    // the real one still works
    assert!(door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(102))).is_ok());
}

#[test]
fn a_session_secret_is_no_link_and_a_link_is_no_session() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let proof = door.link(&entry, "me@example.org", 0).unwrap();
    let entered =
        door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(1))).unwrap();
    let as_link: Proof = entered.secret.reveal().parse().unwrap();
    assert!(door.world.run(entry.peek(&as_link, at(2))).unwrap().is_none());
    let as_session: Proof = proof.reveal().parse().unwrap();
    assert!(door.world.run(entry.resolve(&as_session, at(2))).unwrap().is_none());
}

#[test]
fn nothing_secret_is_in_a_row() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let proof = door.link(&entry, "me@example.org", 0).unwrap();
    let entered =
        door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(1))).unwrap();
    let secrets = [proof.reveal().to_owned(), entered.secret.reveal().to_owned()];
    let mut dump = String::new();
    for select in [
        Query::select()
            .columns([table::Challenge::Digest, table::Challenge::Address])
            .from(table::Challenge::Table)
            .to_owned(),
        Query::select().columns([table::Session::Digest, table::Session::Id]).from(table::Session::Table).to_owned(),
    ] {
        for row in door.rows(&mut select.clone()) {
            for (_, value) in row.columns() {
                write!(dump, "{value:?}").unwrap();
            }
        }
    }
    for secret in secrets {
        assert!(!dump.contains(&secret), "a secret is in a row");
    }
    assert!(!dump.is_empty());
}

#[test]
fn the_sign_up_policy_says_who_makes_an_account() {
    let door = Door::new();
    let closed = door.with(settings(Signup::Closed));
    assert!(door.link(&closed, "a@example.org", 0).is_none());
    assert_eq!(door.world.count(table::Challenge::Table), 0);
    let open = door.with(settings(Signup::Open));
    for who in ["a@example.org", "b@example.org"] {
        let proof = door.link(&open, who, 1).unwrap();
        assert!(
            door.world
                .run(open.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(2)))
                .unwrap()
                .created
        );
    }
    assert_eq!(door.world.count(table::Account::Table), 2);
}

#[test]
fn the_first_policy_lets_one_of_two_pending_links_make_the_account() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let a = door.link(&entry, "a@example.org", 0).unwrap();
    let b = door.link(&entry, "b@example.org", 0).unwrap();
    assert!(door.world.run(entry.redeem(Presented { proof: &a, address: None }, door.drawn(), at(1))).is_ok());
    let refused =
        door.world.run(entry.redeem(Presented { proof: &b, address: None }, door.drawn(), at(2))).unwrap_err();
    assert_eq!(refused.refusal(), Some(&Refused::Closed));
    // refused before it was spent
    assert!(door.world.run(entry.peek(&b, at(3))).unwrap().is_some());
}

#[test]
fn the_first_run_link_is_made_once_and_makes_the_owner() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let issued = door.world.run(entry.admin(None, door.drawn(), at(0))).unwrap().unwrap();
    assert!(door.world.run(entry.admin(None, door.drawn(), at(1))).unwrap().is_none());
    // no address can claim the instance while the link is out
    assert!(door.link(&entry, "thief@example.org", 2).is_none());
    let owner = Door::address("owner@example.org");
    let entered = door
        .world
        .run(entry.redeem(Presented { proof: &issued.proof, address: Some(&owner) }, door.drawn(), at(3)))
        .unwrap();
    assert!(entered.created);
    let again = door
        .world
        .run(entry.redeem(Presented { proof: &issued.proof, address: Some(&owner) }, door.drawn(), at(4)))
        .unwrap_err();
    assert_eq!(again.refusal(), Some(&Refused::Unusable));
    // never made again, even after it was used
    assert!(door.world.run(entry.admin(None, door.drawn(), at(5))).unwrap().is_none());
    // the owner signs in by mail afterwards
    let proof = door.link(&entry, "owner@example.org", 10).unwrap();
    let back = door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(11))).unwrap();
    assert_eq!(back.account, entered.account);
}

#[test]
fn the_first_run_link_can_claim_an_account_that_exists() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let owner: Id<AccountKind> = door.world.id();
    door.world.run(door.world.control.add_account(control::NewAccount { id: owner, at: at(0) })).unwrap();
    let issued = door.world.run(entry.admin(Some(owner), door.drawn(), at(1))).unwrap().unwrap();
    let entered =
        door.world.run(entry.redeem(Presented { proof: &issued.proof, address: None }, door.drawn(), at(2))).unwrap();
    assert_eq!((entered.account, entered.created), (owner, false));
}

#[test]
fn the_first_run_link_ends() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let issued = door.world.run(entry.admin(None, door.drawn(), at(0))).unwrap().unwrap();
    let late = i64::try_from(Settings::default().admin.duration().as_secs()).unwrap();
    let error = door
        .world
        .run(entry.redeem(Presented { proof: &issued.proof, address: None }, door.drawn(), at(late)))
        .unwrap_err();
    assert_eq!(error.refusal(), Some(&Refused::Unusable));
}

fn signed_in(
    door: &Door,
    entry: &SignIn<std::sync::Arc<db_sqlite::Sqlite>>,
    who: &str,
    at_: i64,
) -> control::signin::Entered {
    let proof = door.link(entry, who, at_).unwrap();
    door.world.run(entry.redeem(Presented { proof: &proof, address: None }, door.drawn(), at(at_ + 1))).unwrap()
}

#[test]
fn a_sliding_session_lasts_while_it_is_used_up_to_its_limit() {
    let door = Door::new();
    let settings = Settings {
        session: Expiry::Sliding { idle: "1h".parse().unwrap(), limit: "3h".parse().unwrap() },
        ..Settings::default()
    };
    let entry = door.with(settings);
    let entered = signed_in(&door, &entry, "me@example.org", 0);
    // used every 50 minutes it goes on, until the limit
    for minute in [50, 100, 150] {
        assert!(door.world.run(entry.resolve(&entered.secret, at(minute * 60 + 1))).unwrap().is_some(), "{minute}");
    }
    assert!(door.world.run(entry.resolve(&entered.secret, at(3 * 3600 + 2))).unwrap().is_none());
    // left alone it ends after the idle time
    let other = signed_in(&door, &entry, "me@example.org", 20_000);
    assert!(door.world.run(entry.resolve(&other.secret, at(20_001 + 3601))).unwrap().is_none());
}

#[test]
fn an_absolute_session_ends_when_it_was_told_to_however_it_is_used() {
    let door = Door::new();
    let entry = door.with(Settings { session: Expiry::Absolute { ttl: "1h".parse().unwrap() }, ..Settings::default() });
    let entered = signed_in(&door, &entry, "me@example.org", 0);
    assert!(door.world.run(entry.resolve(&entered.secret, at(3000))).unwrap().is_some());
    assert!(door.world.run(entry.resolve(&entered.secret, at(3602))).unwrap().is_none());
}

#[test]
fn a_session_can_be_revoked_by_its_secret_its_id_or_all_at_once() {
    let door = Door::new();
    let entry = door.with(Settings::default());
    let one = signed_in(&door, &entry, "me@example.org", 0);
    let two = signed_in(&door, &entry, "me@example.org", 10);
    let three = signed_in(&door, &entry, "me@example.org", 20);
    let listed = door.world.run(entry.sessions(one.account, at(30))).unwrap();
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0].id, three.session.id);
    assert!(door.world.run(entry.end(&one.secret, at(31))).unwrap());
    assert!(!door.world.run(entry.end(&one.secret, at(32))).unwrap());
    assert!(door.world.run(entry.resolve(&one.secret, at(33))).unwrap().is_none());
    let stranger: Id<AccountKind> = door.world.id();
    assert!(!door.world.run(entry.revoke(stranger, two.session.id, at(34))).unwrap());
    assert!(door.world.run(entry.revoke(one.account, two.session.id, at(35))).unwrap());
    assert_eq!(door.world.run(entry.revoke_all(one.account, at(36))).unwrap(), 1);
    assert!(door.world.run(entry.sessions(one.account, at(37))).unwrap().is_empty());
    assert!(door.world.run(entry.resolve(&three.secret, at(38))).unwrap().is_none());
}

#[test]
fn ended_rows_are_swept_and_the_first_run_row_is_kept() {
    let door = Door::new();
    let entry = door.with(settings(Signup::Open));
    door.world.run(entry.admin(None, door.drawn(), at(0))).unwrap().unwrap();
    door.link(&entry, "me@example.org", 0);
    assert_eq!(door.world.count(table::Challenge::Table), 2);
    door.world.run(entry.purge(at(10 * 86_400))).unwrap();
    assert_eq!(door.world.count(table::Challenge::Table), 1);
    assert!(door.world.run(entry.admin(None, door.drawn(), at(10 * 86_400 + 1))).unwrap().is_none());
}
