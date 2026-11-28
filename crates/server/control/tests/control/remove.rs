//! Removing a site: bytes before rows, a receipt, a tombstone, and a crash that can be finished.

use std::cell::Cell;
use std::path::Path;

use blob::{Blob, Deleted, Listed, Stored, Tenant};
use blob_dir::{Directory, Error as Fault, Reading};
use build::Source;
use control::table::{
    self, Account, Credential, Deploy, Event, Live, Publication, Removal as Pending, Site, Tombstone,
};
use control::{NewAccount, NewCredential, NewDeploy, NewSite, Policy, Ports, Purpose, Refusal, Removal, SiteState};
use model::Hash;
use sea_query::{Expr, ExprTrait};
use secret::Plain;

use crate::world::{World, at};

#[test]
fn removes_every_row_and_byte_of_a_site_and_nothing_of_another() {
    let world = World::new();
    let site = world.site("cstef");
    let other = world.site("other");
    for (site, texts) in [(&site, [("/", "one"), ("/a", "two")]), (&other, [("/", "keep"), ("/a", "keep too")])] {
        let manifest = world.stage(site, &texts);
        world.run(world.control.deploy(NewDeploy { site: site.id, manifest, build: None, at: at(10) })).unwrap();
    }
    world
        .run(world.control.add_credential(NewCredential {
            id: world.credential(),
            site: site.id,
            purpose: Purpose::GitToken,
            secret: Plain::from("token"),
            at: at(11),
        }))
        .unwrap();
    let receipt = world.run(world.control.remove_site(site.id, at(100))).unwrap();
    assert_eq!((receipt.deploys, receipt.credentials, receipt.builds), (1, 1, 0));
    assert_eq!(receipt.handle.as_str(), "cstef");
    // two files and the manifest, in the site's own tenant
    assert_eq!(receipt.blobs, 3);
    assert!(receipt.to_string().contains("cstef"));
    let tenant = Tenant::try_from(site.id).unwrap();
    assert!(world.run(world.control.ports().blob().list(&tenant)).unwrap().is_empty());
    let gone = world.run(world.control.site(site.id)).unwrap_err();
    assert!(matches!(gone.refusal(), Some(Refusal::NoSite { .. })));
    // only the other site's rows are left
    assert_eq!(world.count(Site::Table), 1);
    assert_eq!(world.count(table::Source::Table), 1);
    assert_eq!(world.count(Deploy::Table), 1);
    assert_eq!(world.count(Live::Table), 1);
    assert_eq!(world.count(Publication::Table), 1);
    assert_eq!(world.count(Credential::Table), 0);
    assert_eq!(world.count(Tombstone::Table), 1);
    assert_eq!(world.counted(Event::Table, Expr::col(Event::SiteId).eq(site.id.to_string())), 0);
    let kept = Tenant::try_from(other.id).unwrap();
    assert_eq!(world.run(world.control.ports().blob().list(&kept)).unwrap().len(), 3);
    assert!(world.run(world.control.live(other.id)).unwrap().is_some());
}

#[test]
fn the_handle_of_a_removed_site_is_not_free() {
    let world = World::new();
    let site = world.site("cstef");
    world.run(world.control.remove_site(site.id, at(100))).unwrap();
    let again = world.run(world.control.add_site(NewSite {
        id: world.id(),
        account: site.account,
        handle: "cstef".parse().unwrap(),
        source: Source::Folder("/srv".into()),
        at: at(200),
    }));
    assert!(matches!(again.unwrap_err().refusal(), Some(Refusal::Held { .. })));
    // the account survives its site
    assert_eq!(world.count(Account::Table), 1);
}

/// A directory whose deletes fail after a few.
struct Flaky {
    inner: Directory,
    deletes: Cell<usize>,
}

impl Blob for Flaky {
    type Error = Fault;
    type Read = Reading;

    async fn put(
        &self,
        t: &Tenant,
        h: Hash,
        c: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Stored, blob::Error<Fault>> {
        self.inner.put(t, h, c).await
    }

    async fn get(&self, t: &Tenant, h: Hash) -> Result<Option<Reading>, blob::Error<Fault>> {
        self.inner.get(t, h).await
    }

    async fn stat(&self, t: &Tenant, h: Hash) -> Result<Option<model::Bytes>, blob::Error<Fault>> {
        self.inner.stat(t, h).await
    }

    async fn list(&self, t: &Tenant) -> Result<Vec<Listed>, blob::Error<Fault>> {
        self.inner.list(t).await
    }

    async fn delete(&self, t: &Tenant, h: Hash) -> Result<Deleted, blob::Error<Fault>> {
        match self.deletes.get() {
            0 => Err(blob::Error::Backend {
                source: Fault::Remove {
                    path: Path::new("injected").to_path_buf(),
                    source: std::io::Error::other("injected"),
                },
            }),
            n => {
                self.deletes.set(n - 1);
                self.inner.delete(t, h).await
            }
        }
    }
}

#[test]
fn a_crash_between_bytes_and_rows_leaves_a_marked_site_and_removing_again_finishes_it() {
    let folder = tempfile::tempdir().unwrap();
    let flaky = Flaky { inner: Directory::open(folder.path()).unwrap(), deletes: Cell::new(1) };
    let control = World::over(flaky);
    let account = world_id(1);
    pollster::block_on(control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
    let id = world_id(2);
    let site = pollster::block_on(control.add_site(NewSite {
        id,
        account,
        handle: "cstef".parse().unwrap(),
        source: Source::Folder("/srv".into()),
        at: at(1),
    }))
    .unwrap();
    let tenant = Tenant::try_from(site.id).unwrap();
    let mut entries = Vec::new();
    for (route, text) in [("/", "one"), ("/a", "two")] {
        let hash = Hash::of(text);
        pollster::block_on(control.ports().blob().put(&tenant, hash, [text.as_bytes()])).unwrap();
        entries.push(model::Served::new(model::Route::parse(route).unwrap(), hash, model::MediaType::Html, vec![]));
    }
    let manifest = model::Manifest::new(entries).unwrap();
    pollster::block_on(control.deploy(NewDeploy { site: id, manifest, build: None, at: at(10) })).unwrap();
    let failed = pollster::block_on(control.remove_site(id, at(100))).unwrap_err();
    assert!(matches!(failed, control::Error::Blob(blob::Error::Backend { .. })), "{failed:?}");
    // the site serves nothing, is marked, and still has its rows: diagnosable, not orphaned bytes
    let marked = pollster::block_on(control.site(id)).unwrap();
    assert_eq!(marked.state, SiteState::Removing);
    assert_eq!(pollster::block_on(control.live(id)).unwrap(), None);
    assert_eq!(pollster::block_on(control.deploys(id, 10)).unwrap().len(), 1);
    let refused = pollster::block_on(control.deploy(NewDeploy {
        site: id,
        manifest: model::Manifest::default(),
        build: None,
        at: at(101),
    }))
    .unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::Removing { .. })));
    // fix the store and remove again: it finishes, and counts only what was left
    control.ports().blob().deletes.set(usize::MAX);
    let receipt = pollster::block_on(control.remove_site(id, at(200))).unwrap();
    assert_eq!(receipt.deploys, 1);
    assert_eq!(receipt.blobs, 3, "the receipt counts the failed run's blob too");
    assert!(receipt.bytes.count() > 6, "the two files and the manifest: {}", receipt.bytes);
    assert!(pollster::block_on(control.site(id)).is_err());
}

fn world_id<K>(n: u8) -> control::Id<K> {
    control::Id::from_bytes([n; 16])
}

/// A site with `files` pages of one deploy, under a policy that deletes `step` blobs at most.
fn stepped(step: u64, files: usize) -> (World, control::Site) {
    let world = World::new().under(Policy::DEFAULT.stepping(step));
    let site = world.site("cstef");
    let texts: Vec<(String, String)> = (0..files).map(|n| (format!("/p{n}"), format!("page {n}"))).collect();
    let pages: Vec<(&str, &str)> = texts.iter().map(|(route, text)| (route.as_str(), text.as_str())).collect();
    let manifest = world.stage(&site, &pages);
    world.run(world.control.deploy(NewDeploy { site: site.id, manifest, build: None, at: at(10) })).unwrap();
    (world, site)
}

#[test]
fn a_removal_runs_in_bounded_steps_and_the_receipt_counts_them_all() {
    // four files and the manifest are five blobs: steps of two are 2, 2 and a last of one
    let (world, site) = stepped(2, 4);
    let tenant = Tenant::try_from(site.id).unwrap();
    let left = || world.run(world.control.ports().blob().list(&tenant)).unwrap().len();
    assert_eq!(left(), 5);
    let Removal::More(first) = world.run(world.control.remove_step(site.id, at(100))).unwrap() else {
        panic!("a first step with more to do")
    };
    assert_eq!(first.blobs, 2);
    assert_eq!(left(), 3, "a step deletes at most its share");
    // between steps the site is marked, serves nothing and has its removal row
    assert_eq!(world.run(world.control.site(site.id)).unwrap().state, SiteState::Removing);
    assert_eq!(world.count(Pending::Table), 1);
    let Removal::More(second) = world.run(world.control.remove_step(site.id, at(101))).unwrap() else {
        panic!("a second step with more to do")
    };
    assert_eq!((second.blobs, left()), (4, 1));
    let Removal::Done(receipt) = world.run(world.control.remove_step(site.id, at(102))).unwrap() else {
        panic!("the last step")
    };
    assert_eq!((receipt.blobs, receipt.deploys, receipt.removed), (5, 1, at(102)));
    assert!(second.bytes.count() < receipt.bytes.count());
    assert_eq!(world.count(Pending::Table), 0, "the removal row goes with the site");
    assert_eq!(world.count(Site::Table), 0);
    assert_eq!(left(), 0);
}

#[test]
fn a_step_of_an_unknown_site_is_refused() {
    let world = World::new();
    let refused = world.run(world.control.remove_step(world.id(), at(1))).unwrap_err();
    assert!(matches!(refused.refusal(), Some(Refusal::NoSite { .. })), "{refused:?}");
}

#[test]
fn a_removal_that_fails_partway_resumes_with_a_whole_receipt() {
    let folder = tempfile::tempdir().unwrap();
    // steps of two, and a store that fails on its fourth delete
    let flaky = Flaky { inner: Directory::open(folder.path()).unwrap(), deletes: Cell::new(3) };
    let control = World::over(flaky).with_policy(Policy::DEFAULT.stepping(2));
    let account = world_id(1);
    pollster::block_on(control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
    let id = world_id(2);
    let site = pollster::block_on(control.add_site(NewSite {
        id,
        account,
        handle: "cstef".parse().unwrap(),
        source: Source::Folder("/srv".into()),
        at: at(1),
    }))
    .unwrap();
    let tenant = Tenant::try_from(site.id).unwrap();
    let mut entries = Vec::new();
    for n in 0..4 {
        let text = format!("page {n}");
        let hash = Hash::of(&text);
        pollster::block_on(control.ports().blob().put(&tenant, hash, [text.as_bytes()])).unwrap();
        let route = model::Route::parse(format!("/p{n}")).unwrap();
        entries.push(model::Served::new(route, hash, model::MediaType::Html, vec![]));
    }
    let manifest = model::Manifest::new(entries).unwrap();
    pollster::block_on(control.deploy(NewDeploy { site: id, manifest, build: None, at: at(10) })).unwrap();
    // the first step deletes two, the second deletes one and then the store fails
    assert!(matches!(pollster::block_on(control.remove_step(id, at(100))).unwrap(), Removal::More(_)));
    let failed = pollster::block_on(control.remove_step(id, at(101))).unwrap_err();
    assert!(matches!(failed, control::Error::Blob(_)), "{failed:?}");
    let left = pollster::block_on(control.ports().blob().list(&tenant)).unwrap().len();
    assert_eq!(left, 2, "three of five blobs are gone, including the one deleted before the failure");
    control.ports().blob().deletes.set(usize::MAX);
    let receipt = pollster::block_on(control.remove_site(id, at(200))).unwrap();
    assert_eq!((receipt.blobs, receipt.deploys), (5, 1), "every step counted, the failed one too");
}
