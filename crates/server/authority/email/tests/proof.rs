//! The email proof over the sign-in rows on SQLite: it works once, ends at 15 minutes, signs
//! nobody in, and is not a sign-in link.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise and read top to bottom")]

extern crate uses_ink_authority_email as authority_email;

use std::sync::Arc;

use authority_email::Email;
use control::signin::{Drawn, Presented, Proof, Refused, Requested, Settings, SignIn, Signup};
use control::{Domain, Remembered, Treed};
use db::Steps;
use db_sqlite::Sqlite;
use jiff::Timestamp;
use mail::{Address, Capture, LinkBase, Template};
use queue_rows::Queued;

type Rows = SignIn<Arc<Sqlite>>;

fn at(n: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + n).unwrap()
}

struct World {
    rows: Rows,
    mail: Capture,
    next: std::cell::Cell<u8>,
}

impl World {
    fn new() -> Self {
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = control::Schema::after(
            Steps::new()
                .with::<Domain>()
                .unwrap()
                .with::<Remembered>()
                .unwrap()
                .with::<Treed>()
                .unwrap()
                .with::<Queued>()
                .unwrap(),
        )
        .unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let settings = Settings { signup: Signup::Open, ..Settings::default() };
        Self { rows: SignIn::new(db, settings), mail: Capture::new(), next: std::cell::Cell::new(1) }
    }

    fn email(&self) -> Email<&Capture> {
        Email::new(&self.mail, "https://ink.example/confirm".parse::<LinkBase>().unwrap())
    }

    fn bytes(&self) -> u8 {
        let n = self.next.get();
        self.next.set(n + 1);
        n
    }

    fn proof(&self) -> Proof {
        let n = self.bytes();
        Proof::from_bytes([[n; 16], [n.wrapping_add(100); 16]])
    }

    fn drawn(&self) -> Drawn {
        let n = self.bytes();
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

    /// Sends a proof to `who` at `now` and returns the token the mailed link carries.
    fn send(&self, who: &str, now: i64) -> Proof {
        pollster::block_on(self.email().send(&self.rows, &Self::address(who), (self.proof(), at(now)))).unwrap();
        let sent = self.mail.sent();
        let Template::Prove { link, .. } = &sent.last().unwrap().template else { panic!("not a proof mail") };
        link.to_string().rsplit('/').next().unwrap().parse().unwrap()
    }
}

#[test]
fn a_proof_is_mailed_as_a_link_and_confirms_the_address_once() {
    let world = World::new();
    let token = world.send("Ada@Example.org", 0);
    let sent = world.mail.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to.as_str(), "ada@example.org");
    assert!(sent[0].template.to_string().contains("15 minutes"));
    assert!(sent[0].template.to_string().contains("does not sign you in"));
    let email = world.email();
    let address = pollster::block_on(email.confirm(&world.rows, &token, at(60))).unwrap();
    assert_eq!(address.as_str(), "ada@example.org");
    // it works once
    let again = pollster::block_on(email.confirm(&world.rows, &token, at(61))).unwrap_err();
    assert_eq!(again.refusal(), Some(&Refused::Unusable));
}

#[test]
fn a_proof_works_for_fifteen_minutes_and_not_a_second_more() {
    let world = World::new();
    let email = world.email();
    let early = world.send("a@example.org", 0);
    let late = world.send("b@example.org", 0);
    let last = 15 * 60 - 1;
    assert!(pollster::block_on(email.confirm(&world.rows, &early, at(last))).is_ok());
    let error = pollster::block_on(email.confirm(&world.rows, &late, at(15 * 60))).unwrap_err();
    assert_eq!(error.refusal(), Some(&Refused::Unusable));
}

#[test]
fn a_proof_is_no_sign_in_and_a_sign_in_link_is_no_proof() {
    let world = World::new();
    let email = world.email();
    let address = World::address("c@example.org");
    // the proof is not redeemable as a sign-in
    let token = world.send("c@example.org", 0);
    let signed =
        pollster::block_on(world.rows.redeem(Presented { proof: &token, address: None }, world.drawn(), at(1)));
    assert_eq!(signed.unwrap_err().refusal(), Some(&Refused::Unusable));
    // it is still spendable as a proof: the failed sign-in did not use it
    assert!(pollster::block_on(email.confirm(&world.rows, &token, at(2))).is_ok());
    // a sign-in link is not a proof
    let Requested::Issued(link) = pollster::block_on(world.rows.request(&address, world.drawn(), at(3))).unwrap()
    else {
        panic!("declined")
    };
    let error = pollster::block_on(email.confirm(&world.rows, &link.proof, at(4))).unwrap_err();
    assert_eq!(error.refusal(), Some(&Refused::Unusable));
    // and the link still signs in after the failed confirmation
    let entered =
        pollster::block_on(world.rows.redeem(Presented { proof: &link.proof, address: None }, world.drawn(), at(5)));
    assert!(entered.is_ok());
}

#[test]
fn a_proof_that_was_never_made_is_refused_like_a_used_one() {
    let world = World::new();
    let email = world.email();
    let stranger = world.proof();
    let error = pollster::block_on(email.confirm(&world.rows, &stranger, at(0))).unwrap_err();
    assert_eq!(error.refusal(), Some(&Refused::Unusable));
}
