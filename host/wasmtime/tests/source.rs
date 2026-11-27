//! The git source as a component against its native core: the component reads the repository
//! through the host's `git` capability and must give the snapshot the core gives over the same
//! fetching. The component is the one `cargo xtask component` wrote to `target/plugin/`.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_wasmtime as host_wasmtime;

#[path = "../../native/tests/support/origin.rs"]
mod origin;

use std::path::Path;

use host_native::{Cloning, Memory};
use host_wasmtime::{Budget, Runtime, Terms, Unsourced};
use model::git::{History, Network, Refused, Want};
use origin::Origin;
use source_git::Source;

fn component() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/plugin/source-git.wasm");
    std::fs::read(&path).unwrap_or_else(|_| panic!("{} is missing: run `cargo xtask component`", path.display()))
}

fn want(origin: &Origin, history: History) -> Want {
    let mut want = Want::new(origin.locator());
    want.history = history;
    want
}

#[test]
fn the_component_reads_the_snapshot_the_native_core_reads() {
    for (name, history) in [("wasm-full", History::Wanted), ("wasm-snapshot", History::Unneeded)] {
        let origin = Origin::new(name);
        let runtime = Runtime::new(Budget::default(), None).unwrap();
        let plugin = runtime
            .source("ink:source-git", &component(), Terms { fetch: origin.fetch(), budget: Budget::default() })
            .unwrap();
        let (mut stored, mut native) = (Memory::default(), Memory::default());
        let read = plugin.read(&want(&origin, history), &mut stored).unwrap();
        let fetch = origin.fetch();
        let direct = Source(Cloning::new(&fetch, &mut native)).read(&want(&origin, history)).unwrap();
        assert_eq!(read.snapshot, direct, "{name}");
        assert_eq!(stored.len(), native.len(), "{name}");
    }
}

#[test]
fn what_the_host_refuses_comes_back_as_the_typed_refusal() {
    let origin = Origin::new("wasm-off");
    let mut fetch = origin.fetch();
    fetch.network = Network::Off;
    let runtime = Runtime::new(Budget::default(), None).unwrap();
    let plugin = runtime.source("ink:source-git", &component(), Terms { fetch, budget: Budget::default() }).unwrap();
    let refused = plugin.read(&want(&origin, History::Wanted), &mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Unsourced::Refused(Refused::Network { .. })), "{refused:?}");
}
