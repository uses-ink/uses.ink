//! Credentials: sealed at rest, bound to their row, never printed.

use control::{Ports, NewCredential, Purpose, Refusal};
use db::{Db, Statement};
use secret::Plain;

use crate::world::{World, at};

fn new(world: &World, site: &control::Site, secret: &str) -> NewCredential {
    NewCredential { id: world.credential(), site: site.id, purpose: Purpose::GitToken, secret: Plain::from(secret), at: at(5) }
}

#[test]
fn stores_a_credential_sealed_and_opens_it_again() {
    let world = World::new();
    let site = world.site("cstef");
    let new = new(&world, &site, "ghp_secret_token");
    let id = new.id;
    let stored = world.run(world.control.add_credential(new)).unwrap();
    assert_eq!((stored.id, stored.site, stored.purpose), (id, site.id, Purpose::GitToken));
    assert_eq!(world.run(world.control.credential(id)).unwrap().expose(), b"ghp_secret_token");
    assert_eq!(world.run(world.control.credentials(site.id)).unwrap(), [stored]);
    let raw: Vec<u8> = world.rows("SELECT sealed FROM credential").first().unwrap().get("sealed").unwrap();
    assert!(!raw.windows(8).any(|w| w == b"ghp_secr"), "the secret is in the row in the clear");
}

#[test]
fn a_sealed_value_copied_to_another_row_does_not_open() {
    let world = World::new();
    let site = world.site("cstef");
    let (first, second) = (new(&world, &site, "one"), new(&world, &site, "two"));
    let (a, b) = (first.id, second.id);
    world.run(world.control.add_credential(first)).unwrap();
    world.run(world.control.add_credential(second)).unwrap();
    let copy = Statement::prepare("UPDATE credential SET sealed = (SELECT sealed FROM credential WHERE id = ?) WHERE id = ?")
        .bind(a.to_string())
        .bind(b.to_string());
    world.run(world.control.ports().db().run(copy)).unwrap();
    let opened = world.run(world.control.credential(b)).unwrap_err();
    assert!(matches!(opened, control::Error::Secret(secret::Error::Refused)), "{opened:?}");
    assert_eq!(world.run(world.control.credential(a)).unwrap().expose(), b"one");
}

#[test]
fn nothing_about_a_credential_prints_its_secret() {
    let world = World::new();
    let site = world.site("cstef");
    let new = new(&world, &site, "hunter2-secret");
    assert!(!format!("{new:?}").contains("hunter2"));
    let stored = world.run(world.control.add_credential(new)).unwrap();
    assert!(!format!("{stored:?}").contains("hunter2"));
}

#[test]
fn refuses_what_has_no_site_or_no_credential() {
    let world = World::new();
    let site = world.site("cstef");
    let mut stray = new(&world, &site, "x");
    stray.site = world.id();
    assert!(matches!(world.run(world.control.add_credential(stray)).unwrap_err().refusal(), Some(Refusal::NoSite { .. })));
    let missing = world.run(world.control.credential(world.credential())).unwrap_err();
    assert!(matches!(missing.refusal(), Some(Refusal::NoCredential { .. })));
}
