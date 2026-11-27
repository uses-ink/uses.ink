//! The git source as a component against its native core: the component reads the repository
//! through the host's `git` capability and must give the snapshot the core gives over the same
//! fetching. The component is the one `cargo xtask component` wrote to `target/plugin/`.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_wasmtime as host_wasmtime;

use std::path::Path;
use std::time::Duration;

use fixture::origin::Origin;
use host_native::{Cloning, Fetch, Memory};
use host_wasmtime::{Budget, Error, Runtime, Terms, Unsourced};
use model::git::{History, Network, Refused, Want};
use source_git::Source;

fn component() -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/plugin/source-git.wasm");
    std::fs::read(&path).unwrap_or_else(|_| panic!("{} is missing: run `cargo xtask component`", path.display()))
}

fn origin(name: &str) -> Origin {
    Origin::new(Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-wasmtime-source").join(name))
}

fn fetch(origin: &Origin) -> Fetch {
    Fetch { network: Network::Local, ..Fetch::new(&origin.cache) }
}

fn want(origin: &Origin, history: History) -> Want {
    let mut want = Want::new(origin.url().parse().unwrap());
    want.history = history;
    want
}

#[test]
fn the_component_reads_the_snapshot_the_native_core_reads() {
    for (name, history) in [("wasm-full", History::Wanted), ("wasm-snapshot", History::Unneeded)] {
        let origin = origin(name);
        let runtime = Runtime::new(Budget::default(), None).unwrap();
        let plugin = runtime
            .source("ink:source-git", &component(), Terms { fetch: fetch(&origin), budget: Budget::default() })
            .unwrap();
        let (mut stored, mut native) = (Memory::default(), Memory::default());
        let read = plugin.read(&want(&origin, history), &mut stored).unwrap();
        let fetch = fetch(&origin);
        let direct = Source(Cloning::new(&fetch, &mut native)).read(&want(&origin, history)).unwrap();
        assert_eq!(read.snapshot, direct, "{name}");
        assert_eq!(stored.len(), native.len(), "{name}");
    }
}

#[test]
fn what_the_host_refuses_comes_back_as_the_typed_refusal() {
    let origin = origin("wasm-off");
    let mut fetch = fetch(&origin);
    fetch.network = Network::Off;
    let runtime = Runtime::new(Budget::default(), None).unwrap();
    let plugin = runtime.source("ink:source-git", &component(), Terms { fetch, budget: Budget::default() }).unwrap();
    let refused = plugin.read(&want(&origin, History::Wanted), &mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Unsourced::Refused(Refused::Network { .. })), "{refused:?}");
}

#[test]
fn terms_in_another_tick_than_the_runtime_are_refused() {
    let origin = origin("wasm-tick");
    let runtime = Runtime::new(Budget::default(), None).unwrap();
    for tick in [Duration::from_millis(1), Duration::ZERO] {
        let terms = Terms { fetch: fetch(&origin), budget: Budget { tick, ..Budget::default() } };
        let refused = runtime.source("ink:source-git", &component(), terms).unwrap_err();
        assert!(matches!(refused, Error::TermsTick { terms, .. } if terms == tick), "{refused:?}");
    }
}
