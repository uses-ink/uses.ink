//! A control plane on real adapters, and the fixtures the tests use.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;

use blob::{Blob, Tenant};
use blob_dir::Directory;
use build::Source;
use control::{Ports, AccountKind, Control, Origin, CredentialKind, Handle, Id, NewAccount, NewSite, Site, SiteKind};
use db::{Db, Row, Statement, Steps};
use db_sqlite::Sqlite;
use jiff::Timestamp;
use model::{Hash, Manifest, MediaType, Route, Served};
use queue_local::Local as Queue;
use secret_local::Local as Sealer;

/// The ports of a test world: the database is shared with the queue.
pub type Wired = (Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer);

/// A control plane, and the folder its blobs are in.
pub struct World {
    pub control: Control<Wired>,
    pub root: PathBuf,
    folder: tempfile::TempDir,
    next: Cell<u8>,
}

/// The time `n` seconds after an arbitrary start.
pub fn at(n: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + n).unwrap()
}

impl World {
    /// Where the test sites are served.
    fn origin() -> Origin {
        Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap()
    }

    /// A control plane with every migration applied.
    pub fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("blobs");
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new(control::MIGRATIONS.into_iter().chain(Queue::<Arc<Sqlite>>::STEPS)).unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (db.clone(), Directory::open(&root).unwrap(), Queue::new(db), Sealer::ephemeral().unwrap());
        Self { control: Control::new(ports, Self::origin()), root, folder, next: Cell::new(1) }
    }

    /// A control plane over these ports instead, with every migration applied.
    pub fn over<B: Blob>(blob: B) -> Control<(Arc<Sqlite>, B, Queue<Arc<Sqlite>>, Sealer)> {
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new(control::MIGRATIONS.into_iter().chain(Queue::<Arc<Sqlite>>::STEPS)).unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        Control::new((db.clone(), blob, Queue::new(db), Sealer::ephemeral().unwrap()), Self::origin())
    }

    /// A fresh id: the same calls in the same order draw the same ids.
    pub fn id<K>(&self) -> Id<K> {
        let n = self.next.get();
        self.next.set(n + 1);
        Id::from_bytes([n; 16])
    }

    pub fn run<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        pollster::block_on(future)
    }

    /// An account and a site of it, with a folder source.
    pub fn site(&self, handle: &str) -> Site {
        let account: Id<AccountKind> = self.id();
        self.run(self.control.add_account(NewAccount { id: account, at: at(0) })).unwrap();
        self.run(self.control.add_site(NewSite {
            id: self.id::<SiteKind>(),
            account,
            handle: handle.parse::<Handle>().unwrap(),
            source: Source::Folder("/srv/blog".into()),
            at: at(1),
        }))
        .unwrap()
    }

    /// Stores `files` (route, bytes) in the site's tenant and returns the manifest that serves
    /// them.
    pub fn stage(&self, site: &Site, files: &[(&str, &str)]) -> Manifest {
        let tenant = Tenant::try_from(site.id).unwrap();
        let entries = files
            .iter()
            .map(|(route, text)| {
                let hash = Hash::of(text);
                self.run(self.control.ports().blob().put(&tenant, hash, [text.as_bytes()])).unwrap();
                Served::new(Route::parse(*route).unwrap(), hash, MediaType::Html, vec![])
            })
            .collect();
        Manifest::new(entries).unwrap()
    }

    pub fn rows(&self, sql: &'static str) -> Vec<Row> {
        self.run(self.control.ports().db().rows(Statement::prepare(sql))).unwrap()
    }

    pub fn count(&self, sql: &'static str) -> u64 {
        self.rows(sql).first().unwrap().get("n").unwrap()
    }

    /// The folder of the world, kept so the throwaway directory lives as long as it.
    pub fn folder(&self) -> &std::path::Path {
        self.folder.path()
    }

    pub fn credential(&self) -> Id<CredentialKind> {
        self.id()
    }
}
