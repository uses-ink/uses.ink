//! Removing a site: bytes before rows, a receipt, a tombstone, and a crash that can be finished.

use std::cell::Cell;
use std::path::Path;

use blob::{Blob, Deleted, Listed, Stored, Tenant};
use blob_dir::{Directory, Error as Fault, Reading};
use build::Source;
use control::{Ports, NewAccount, NewCredential, NewDeploy, NewSite, Purpose, Refusal, SiteState};
use model::Hash;
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
    for (sql, left) in [
        ("SELECT COUNT(*) AS n FROM site", 1),
        ("SELECT COUNT(*) AS n FROM source", 1),
        ("SELECT COUNT(*) AS n FROM deploy", 1),
        ("SELECT COUNT(*) AS n FROM site_live", 1),
        ("SELECT COUNT(*) AS n FROM publication", 1),
        ("SELECT COUNT(*) AS n FROM credential", 0),
        ("SELECT COUNT(*) AS n FROM tombstone", 1),
    ] {
        assert_eq!(world.count(sql), left, "{sql}");
    }
    assert_eq!(world.count("SELECT COUNT(*) AS n FROM event WHERE site_id = lower(hex(zeroblob(16)))"), 0);
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
    assert!(matches!(again.unwrap_err().refusal(), Some(Refusal::Tombstoned { .. })));
    // the account survives its site
    assert_eq!(world.count("SELECT COUNT(*) AS n FROM account"), 1);
}

/// A directory whose deletes fail after a few.
struct Flaky {
    inner: Directory,
    deletes: Cell<usize>,
}

impl Blob for Flaky {
    type Error = Fault;
    type Read = Reading;

    async fn put(&self, t: &Tenant, h: Hash, c: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Result<Stored, blob::Error<Fault>> {
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
                source: Fault::Remove { path: Path::new("injected").to_path_buf(), source: std::io::Error::other("injected") },
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
    assert_eq!(receipt.blobs, 2, "one of three blobs went in the failed run");
    assert!(pollster::block_on(control.site(id)).is_err());
}

fn world_id<K>(n: u8) -> control::Id<K> {
    control::Id::from_bytes([n; 16])
}
