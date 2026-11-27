//! The components against their native cores, and a hostile component against the host's
//! budgets. The components are the ones `cargo xtask component` wrote to `target/plugin/`, as
//! the gate builds them before the tests: built once per run, not per test.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_host_wasmtime as host_wasmtime;

use std::path::{Path, PathBuf};
use std::time::Duration;

use engine::{BaseUrl, Engine, Format as _, NoPins, RenderSettings};
use host_image::Pixels;
use host_native::{Folder, Grammars, Memory};
use host_wasmtime::{Budget, Cache, Component, Format, Origin, Runtime};
use model::{Bytes, ContentPath, Read};
use theme_quiet::Quiet;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn plugin(name: &str) -> Vec<u8> {
    let path = root().join("target/plugin").join(format!("{name}.wasm"));
    std::fs::read(&path).unwrap_or_else(|_| panic!("{} is missing: run `cargo xtask component`", path.display()))
}

fn runtime(budget: Budget) -> Runtime {
    Runtime::new(budget, Some(Cache::new(root().join("target/plugin/cache")))).unwrap()
}

/// The native markdown format, as the engine takes it.
struct Markdown;

impl engine::Format for Markdown {
    fn handles(&self, path: &ContentPath) -> bool {
        format_markdown::handles(path)
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        format_markdown::read(path, bytes)
    }
}

/// The native obsidian format, as the engine takes it.
struct Obsidian;

impl engine::Format for Obsidian {
    fn handles(&self, path: &ContentPath) -> bool {
        format_obsidian::handles(path)
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        format_obsidian::read(path, bytes)
    }
}

/// Reads every fixture of the format in `plugin/format/<variant>/fixture` through its component
/// and through `native`, and compares them.
fn reads_like(variant: &str, native: &impl engine::Format) {
    let runtime = runtime(Budget::default());
    let format = runtime.format(&format!("ink:format-{variant}"), &plugin(&format!("format-{variant}"))).unwrap();
    let dir = root().join("plugin/format").join(variant).join("fixture");
    let mut read = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let file = entry.unwrap().path();
        if file.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let path: ContentPath = file.file_name().unwrap().to_str().unwrap().parse().unwrap();
        let bytes = std::fs::read(&file).unwrap();
        assert!(format.handles(&path));
        assert_eq!(format.read(&path, &bytes), native.read(&path, &bytes), "{path}");
        read += 1;
    }
    assert!(read > 8, "only {read} fixtures");
    assert_eq!(format.aside().diagnostics, []);
}

#[test]
fn the_markdown_component_reads_every_fixture_as_its_native_core() {
    reads_like("markdown", &Markdown);
}

#[test]
fn the_obsidian_component_reads_every_fixture_as_its_native_core() {
    reads_like("obsidian", &Obsidian);
}

#[test]
fn the_quiet_component_renders_a_site_as_its_native_core() {
    let mut store = Memory::default();
    let snapshot = Folder::new(root().join("engine/fixture/site/blog")).read(&mut store).unwrap().snapshot;
    let understood = Engine::new(&store, NoPins, (Markdown,), ()).interpret(&snapshot);
    let site = understood.site.unwrap();
    let base: BaseUrl = "https://example.org".parse().unwrap();
    let settings = RenderSettings { base: base.clone() };

    let native = Quiet::new(base.clone(), Grammars::default());
    let natively = Engine::new(&store, NoPins, (), &native).render(&site, &settings, &Pixels::default());
    let runtime = runtime(Budget::default());
    let theme = runtime.theme(&plugin("theme-quiet"), base).unwrap();
    let through = Engine::new(&store, NoPins, (), &theme).render(&site, &settings, &Pixels::default());

    assert_eq!(engine::Theme::name(&theme), "ink:theme-quiet");
    assert_eq!(through.blobs, natively.blobs);
    assert_eq!(through.manifest.id(), natively.manifest.id());
    assert_eq!(through.diagnostics, natively.diagnostics);
    assert_eq!(through.notices, natively.notices);
}

/// The hostile component, under `budget`.
fn hostile(budget: Budget) -> Component<Format> {
    runtime(budget).format("hostile", &plugin("hostile")).unwrap()
}

/// What reading `stem` gives: the code of its one diagnostic.
fn refusal(format: &Component<Format>, stem: &str) -> &'static str {
    let read = format.read(&format!("{stem}.md").parse().unwrap(), b"");
    assert_eq!(read.diagnostics.len(), 1, "{stem}: {:?}", read.diagnostics);
    read.diagnostics[0].code()
}

/// After any refusal, the next call is served.
fn serves(format: &Component<Format>) {
    let read = format.read(&"fine.md".parse().unwrap(), b"");
    assert!(read.diagnostics.is_empty(), "{:?}", read.diagnostics);
}

#[test]
fn a_spinning_plugin_is_cut_at_its_time() {
    let format = hostile(Budget { time: Duration::from_millis(50), ..Budget::default() });
    let started = std::time::Instant::now();
    assert_eq!(refusal(&format, "spin"), "plugin.budget.time");
    assert!(started.elapsed() < Duration::from_millis(500));
    serves(&format);
}

#[test]
fn a_spinning_plugin_runs_out_of_fuel() {
    let format = hostile(Budget { fuel: 1_000_000, ..Budget::default() });
    assert_eq!(refusal(&format, "spin"), "plugin.budget.fuel");
    serves(&format);
}

#[test]
fn a_growing_plugin_is_refused_memory() {
    let format = hostile(Budget { memory: Bytes(8 << 20), ..Budget::default() });
    assert_eq!(refusal(&format, "grow"), "plugin.budget.memory");
    serves(&format);
}

#[test]
fn a_plugin_that_overflows_its_stack_or_panics_traps() {
    let format = hostile(Budget::default());
    assert_eq!(refusal(&format, "deep"), "plugin.call.trapped");
    assert_eq!(refusal(&format, "panic"), "plugin.call.trapped");
    serves(&format);
}

#[test]
fn a_plugin_that_returns_garbage_is_refused() {
    let format = hostile(Budget::default());
    assert_eq!(refusal(&format, "garbage"), "plugin.value.malformed");
    serves(&format);
}

#[test]
fn a_compiled_component_is_cached_and_a_corrupt_one_replaced() {
    let dir = root().join(format!("target/plugin/cache-test-{}", std::process::id()));
    let load = || {
        let runtime = Runtime::new(Budget::default(), Some(Cache::new(&dir))).unwrap();
        runtime.format("hostile", &plugin("hostile")).unwrap().origin()
    };
    assert_eq!(load(), Origin::Compiled);
    assert_eq!(load(), Origin::Cached);
    for entry in std::fs::read_dir(&dir).unwrap() {
        std::fs::write(entry.unwrap().path(), b"not a cwasm").unwrap();
    }
    assert_eq!(load(), Origin::Replaced);
    assert_eq!(load(), Origin::Cached);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_plugin_that_needs_a_larger_table_than_it_may_have_is_refused() {
    // Rust cannot grow a table at run time, so the limit is set under the table every instance
    // starts with: the instance is refused its table.
    let format = hostile(Budget { table: 0, ..Budget::default() });
    assert_eq!(refusal(&format, "fine"), "plugin.budget.table");
}

#[test]
fn a_cache_entry_written_for_another_plugin_is_replaced() {
    let dir = root().join(format!("target/plugin/cache-seal-{}", std::process::id()));
    let files = || -> std::collections::BTreeSet<PathBuf> {
        std::fs::read_dir(&dir).into_iter().flatten().map(|entry| entry.unwrap().path()).collect()
    };
    let runtime = Runtime::new(Budget::default(), Some(Cache::new(&dir))).unwrap();
    runtime.format("hostile", &plugin("hostile")).unwrap();
    let hostile = files();
    runtime.format("markdown", &plugin("format-markdown")).unwrap();
    let markdown = files();
    let (own, other) = (hostile.iter().next().unwrap(), markdown.difference(&hostile).next().unwrap());
    std::fs::copy(other, own).unwrap();
    assert_eq!(runtime.format("hostile", &plugin("hostile")).unwrap().origin(), Origin::Replaced);
    assert_eq!(runtime.format("hostile", &plugin("hostile")).unwrap().origin(), Origin::Cached);
    std::fs::remove_dir_all(&dir).unwrap();
}
