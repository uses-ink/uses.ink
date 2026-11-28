//! The Worker's serve adapters, `Sited` and `Stored`, against the edge's conformance suite on the
//! native adapters under them: SQLite rows and a directory of blobs. The same suite runs over D1 and
//! R2 in workerd (`cargo xtask worker edge`).

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_edge_worker as edge_worker;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use blob_dir::Directory;
use clock::Manual;
use control::{Domain, Origin, Remembered, Treed};
use db::Steps;
use db_sqlite::Sqlite;
use edge_worker::Hosted;
use jiff::Timestamp;
use queue_rows::{Local as Queue, Queued};
use secret_local::Local as Sealer;

type Wired = Arc<(Arc<Sqlite>, Directory, Queue<Arc<Sqlite>>, Sealer)>;

/// A scratch folder no other test of this run uses, empty.
fn scratch() -> std::path::PathBuf {
    static MADE: AtomicU32 = AtomicU32::new(0);
    let name = format!("serve-{}-{}", std::process::id(), MADE.fetch_add(1, Ordering::Relaxed));
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("edge-worker").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// SQLite rows and a directory of blobs, served through the Worker's adapters.
fn world() -> Hosted<Wired, Arc<Manual>, Directory> {
    let root = scratch().join("blobs");
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
    let ports: Wired =
        Arc::new((db.clone(), Directory::open(&root).unwrap(), Queue::new(db), Sealer::ephemeral().unwrap()));
    let origin = Origin::try_from(&model::ExternalUrl::parse("https://example.test").unwrap()).unwrap();
    let clock = Arc::new(Manual::new(Timestamp::from_second(1_800_000_000).unwrap(), 0));
    Hosted::new(ports, Directory::open(&root).unwrap(), origin, clock)
}

edge_conformance::suite!(world);
