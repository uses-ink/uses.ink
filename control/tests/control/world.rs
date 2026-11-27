//! A control plane on real adapters, and the fixtures the tests use.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;

use blob::{Blob, Tenant};
use blob_dir::Directory;
use build::Source;
use control::signin::Entry;
use control::table::Event;
use control::{
    AccountKind, Control, CredentialKind, Domain, Handle, Id, NewAccount, NewSite, Origin, Ports, Site, SiteKind,
};
use db::{Db, Row, Steps};
use db_sqlite::Sqlite;
use jiff::Timestamp;
use model::{Hash, Manifest, MediaType, Route, Served};
use queue_local::{Local as Queue, Queued};
use sea_query::{Asterisk, Expr, ExprTrait, Func, IdenStatic, IntoTableRef, Query, SelectStatement};
use secret_local::Local as Sealer;

/// The ports of a test world: the database is shared with the queue.
pub(crate) type Wired = Wiring<Directory>;

/// The ports of a test world over a blob store `B`.
pub(crate) type Wiring<B> = (Arc<Sqlite>, B, Queue<Arc<Sqlite>>, Sealer);

/// The one column of a count.
#[derive(Clone, Copy, IdenStatic)]
enum Tally {
    N,
}

/// A control plane, and the folder its blobs are in.
pub(crate) struct World {
    pub(crate) control: Control<Wired>,
    pub(crate) root: PathBuf,
    /// Kept so the throwaway directory lives as long as the world.
    _folder: tempfile::TempDir,
    next: Cell<u8>,
}

/// The time `n` seconds after an arbitrary start.
pub(crate) fn at(n: i64) -> Timestamp {
    Timestamp::from_second(1_800_000_000 + n).unwrap()
}

impl World {
    /// Where the test sites are served.
    fn origin() -> Origin {
        Origin::try_from(&model::ExternalUrl::parse("https://uses.ink").unwrap()).unwrap()
    }

    /// A control plane with every migration applied.
    pub(crate) fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("blobs");
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new().with::<Domain>().unwrap().with::<Queued>().unwrap().with::<Entry>().unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        let ports = (db.clone(), Directory::open(&root).unwrap(), Queue::new(db), Sealer::ephemeral().unwrap());
        Self { control: Control::new(ports, Self::origin()), root, _folder: folder, next: Cell::new(1) }
    }

    /// A control plane over these ports instead, with every migration applied.
    pub(crate) fn over<B: Blob>(blob: B) -> Control<Wiring<B>> {
        let db = Arc::new(Sqlite::memory().unwrap());
        let steps = Steps::new().with::<Domain>().unwrap().with::<Queued>().unwrap().with::<Entry>().unwrap();
        pollster::block_on(steps.apply(&db)).unwrap();
        Control::new((db.clone(), blob, Queue::new(db), Sealer::ephemeral().unwrap()), Self::origin())
    }

    /// A fresh id: the same calls in the same order draw the same ids.
    pub(crate) fn id<K>(&self) -> Id<K> {
        let n = self.next.get();
        self.next.set(n + 1);
        Id::from_bytes([n; 16])
    }

    #[allow(clippy::unused_self, reason = "called on the world so a test reads as one object")]
    pub(crate) fn run<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        pollster::block_on(future)
    }

    /// An account and a site of it, with a folder source.
    pub(crate) fn site(&self, handle: &str) -> Site {
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
    pub(crate) fn stage(&self, site: &Site, files: &[(&str, &str)]) -> Manifest {
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

    pub(crate) fn rows(&self, select: &mut SelectStatement) -> Vec<Row> {
        self.run(self.control.ports().db().rows(select.into())).unwrap()
    }

    /// How many rows `table` has.
    pub(crate) fn count<T: IntoTableRef>(&self, table: T) -> u64 {
        self.counted(table, Expr::val(1_i64).eq(1_i64))
    }

    /// How many rows of `table` satisfy `filter`.
    pub(crate) fn counted<T: IntoTableRef>(&self, table: T, filter: Expr) -> u64 {
        let select = Query::select()
            .expr_as(Func::count(Expr::col(Asterisk)), Tally::N)
            .from(table)
            .and_where(filter)
            .to_owned();
        self.run(self.control.ports().db().first(select.into())).unwrap().unwrap().get(Tally::N).unwrap()
    }

    /// How many events of the log are named `name`.
    pub(crate) fn events(&self, name: &str) -> u64 {
        self.counted(Event::Table, Expr::col(Event::Name).eq(name))
    }

    pub(crate) fn credential(&self) -> Id<CredentialKind> {
        self.id()
    }
}
