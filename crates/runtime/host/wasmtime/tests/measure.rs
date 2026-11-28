//! The numbers `docs/decisions.md` cites for the components: compile and cached load per
//! plugin, and the blog fixture rendered natively and through the components. Run with
//! `cargo test --release -p uses-ink-host-wasmtime --test measure -- --ignored`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a measurement stops at the first surprise"
)]

extern crate uses_ink_host_wasmtime as host_wasmtime;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use engine::{BaseUrl, Engine, NoPins, RenderSettings};
use host_image::Pixels;
use host_native::{Folder, Grammars, Memory};
use host_wasmtime::{Budget, Cache, Origin, Runtime};
use model::{ContentPath, Read};
use theme_quiet::Quiet;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

fn plugin(name: &str) -> Vec<u8> {
    std::fs::read(root().join("target/plugin").join(format!("{name}.wasm"))).unwrap()
}

struct Markdown;

impl engine::Format for Markdown {
    fn handles(&self, path: &ContentPath) -> bool {
        format_markdown::handles(path)
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        format_markdown::read(path, bytes)
    }
}

/// The median of `rounds` runs of `work`.
fn median(rounds: usize, mut work: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..rounds)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .collect();
    times.sort();
    times[rounds / 2]
}

#[test]
#[ignore = "a measurement, not a check: run it by hand in release"]
fn measure() {
    let mut err = std::io::stderr();
    let dir = root().join(format!("target/plugin/measure-{}", std::process::id()));
    for name in ["format-markdown", "theme-quiet"] {
        let bytes = plugin(name);
        let load = |cache: Option<Cache>| {
            let runtime = Runtime::new(Budget::default(), cache).unwrap();
            if name == "theme-quiet" {
                runtime.theme("ink:theme-quiet", &bytes, "https://a.b".parse().unwrap()).unwrap().origin()
            } else {
                runtime.format(name, &bytes).unwrap().origin()
            }
        };
        let compile = median(5, || assert_eq!(load(None), Origin::Compiled));
        load(Some(Cache::new(&dir)));
        let cached = median(21, || assert_eq!(load(Some(Cache::new(&dir))), Origin::Cached));
        writeln!(err, "{name}: {} bytes, compile {compile:?}, cached load {cached:?}", bytes.len()).unwrap();
    }
    std::fs::remove_dir_all(&dir).unwrap();

    let mut store = Memory::default();
    let snapshot = Folder::new(root().join("crates/core/engine/fixture/site/blog")).read(&mut store).unwrap().snapshot;
    let base: BaseUrl = "https://example.org".parse().unwrap();
    let settings = RenderSettings { base: base.clone() };
    let native = Quiet::new(base.clone(), Grammars::default());
    let natively = median(21, || {
        let site = Engine::new(&store, NoPins, (Markdown,), ()).interpret(&snapshot).site.unwrap();
        Engine::new(&store, NoPins, (), &native).render(&site, &settings, &Pixels::default());
    });
    let runtime = Runtime::new(Budget::default(), Some(Cache::new(root().join("target/plugin/cache")))).unwrap();
    let format = runtime.format("ink:format-markdown", &plugin("format-markdown")).unwrap();
    let theme = runtime.theme("ink:theme-quiet", &plugin("theme-quiet"), base).unwrap();
    let through = median(21, || {
        let site = Engine::new(&store, NoPins, (&format,), ()).interpret(&snapshot).site.unwrap();
        Engine::new(&store, NoPins, (), &theme).render(&site, &settings, &Pixels::default());
    });
    writeln!(err, "blog fixture, interpret and render: native {natively:?}, components {through:?}").unwrap();
}
