//! The spike host: links `ink:host/pen` (the real `freehand` crate) and `ink:host/text` (a crude
//! stub) into the theme component, renders one page, and times compile, instantiation and calls.
//!
//! `cargo run --release -- <component.wasm> <out-dir>`

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, InstanceAllocationStrategy, PoolingAllocationConfig, Store};

wasmtime::component::bindgen!({
    path: "../wit",
    world: "ink:theme/theme",
});

use ink::host::{pen, text};

/// What a store holds: nothing yet. A real host keeps the blob reads and diagnostics here.
struct Host;

impl pen::Host for Host {
    fn stroke(&mut self, start: pen::Point, end: pen::Point, weight: f64, seed: String) -> String {
        let from = freehand::Point::new(start.x, start.y);
        let to = freehand::Point::new(end.x, end.y);
        freehand::mark::line(from, to, weight, &seed).as_str().to_owned()
    }
}

impl text::Host for Host {
    /// Stub: half an em per char, 1.2 em high. The real one shapes with harfrust.
    #[allow(clippy::cast_precision_loss, reason = "a stub; char counts are small")]
    fn measure(&mut self, _font: String, size: f64, text: String) -> text::Extent {
        text::Extent { width: text.chars().count() as f64 * size * 0.5, height: size * 1.2 }
    }
}

impl ink::theme::page::Host for Host {}

/// Fuel and epoch deadline far above anything the bench uses: budgets on, never hit.
const FUEL: u64 = u64::MAX / 2;

#[derive(Clone, Copy)]
enum Allocator {
    OnDemand,
    Pooling,
    /// Pooling, and a slot keeps its first MiB of memory mapped when it is reused.
    PoolingResident,
}

fn engine(allocator: Allocator, budgets: bool) -> wasmtime::Result<Engine> {
    let mut config = Config::new();
    config.consume_fuel(budgets).epoch_interruption(budgets);
    if !matches!(allocator, Allocator::OnDemand) {
        let mut pool = PoolingAllocationConfig::new();
        pool.total_component_instances(1000)
            .total_core_instances(1000)
            .total_memories(1000)
            .total_tables(1000)
            .max_memory_size(64 << 20);
        if matches!(allocator, Allocator::PoolingResident) {
            pool.linear_memory_keep_resident(1 << 20).table_keep_resident(1 << 16);
        }
        config.allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
    }
    Engine::new(&config)
}

fn store(engine: &Engine, budgets: bool) -> wasmtime::Result<Store<Host>> {
    let mut store = Store::new(engine, Host);
    if budgets {
        store.set_fuel(FUEL)?;
        store.set_epoch_deadline(u64::MAX / 2);
    }
    Ok(store)
}

fn linker(engine: &Engine) -> wasmtime::Result<Linker<Host>> {
    let mut linker = Linker::new(engine);
    Theme::add_to_linker::<_, HasSelf<_>>(&mut linker, |host| host)?;
    Ok(linker)
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

/// Median of `rounds` timings of `f`.
fn time<T>(rounds: usize, mut f: impl FnMut() -> wasmtime::Result<T>) -> wasmtime::Result<Duration> {
    let mut samples = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        std::hint::black_box(f()?);
        samples.push(start.elapsed());
    }
    Ok(median(samples))
}

fn show(label: &str, d: Duration) {
    println!("{label:<52} {:>12.3} us", d.as_secs_f64() * 1e6);
}

fn show_ns(label: &str, d: Duration, per: u32) {
    println!("{label:<52} {:>12.1} ns", d.as_secs_f64() * 1e9 / f64::from(per));
}

fn page() -> Page {
    Page {
        title: "Notes from the margin".into(),
        paragraphs: vec![
            "A theme renders pages; the engine owns the site.".into(),
            "This paragraph & that one are escaped by unspliced.".into(),
        ],
    }
}

fn run(wasm: &Path, out: &Path) -> wasmtime::Result<()> {
    let bytes = std::fs::read(wasm)?;
    println!("component {} ({} bytes)", wasm.display(), bytes.len());

    // Compile with Cranelift: what a first build of a plugin version pays.
    let plain = engine(Allocator::OnDemand, true)?;
    show("compile (Component::new, cranelift)", time(10, || Component::new(&plain, &bytes))?);

    // Precompile once, then load from bytes and from a file.
    let cwasm = plain.precompile_component(&bytes)?;
    let cwasm_path = out.join("theme.cwasm");
    std::fs::write(&cwasm_path, &cwasm)?;
    println!("cwasm {} ({} bytes)", cwasm_path.display(), cwasm.len());
    // SAFETY: the bytes come from `precompile_component` on this engine's config, a moment ago.
    show("deserialize (.cwasm bytes)", time(200, || unsafe { Component::deserialize(&plain, &cwasm) })?);
    // SAFETY: as above; the file is the one just written.
    show("deserialize_file (.cwasm, mmap)", time(200, || unsafe { Component::deserialize_file(&plain, &cwasm_path) })?);

    // Cold: a fresh engine loads the .cwasm file, links, instantiates and renders once.
    show(
        "cold: new engine + .cwasm file + link + render",
        time(50, || {
            let engine = engine(Allocator::OnDemand, true)?;
            // SAFETY: the file was written by `precompile_component` above, same config.
            let component = unsafe { Component::deserialize_file(&engine, &cwasm_path) }?;
            let mut store = store(&engine, true)?;
            let theme = Theme::instantiate(&mut store, &component, &linker(&engine)?)?;
            theme.call_render(&mut store, &page())
        })?,
    );

    for (name, allocator) in [
        ("on-demand", Allocator::OnDemand),
        ("pooling", Allocator::Pooling),
        ("pooling, 1 MiB resident", Allocator::PoolingResident),
    ] {
        let engine = engine(allocator, true)?;
        // SAFETY: produced by `precompile_component` with the same config apart from the
        // allocator, which wasmtime does not hash into compatibility.
        let component = unsafe { Component::deserialize(&engine, &cwasm) }?;
        let linker = linker(&engine)?;
        show(
            &format!("{name}: link + instantiate (no InstancePre)"),
            time(2000, || {
                let mut store = store(&engine, true)?;
                linker.instantiate(&mut store, &component)
            })?,
        );
        let pre = ThemePre::new(linker.instantiate_pre(&component)?)?;
        show(
            &format!("{name}: instantiate from InstancePre"),
            time(20000, || {
                let mut store = store(&engine, true)?;
                pre.instantiate(&mut store)
            })?,
        );
        show(
            &format!("{name}: instantiate + render one page"),
            time(5000, || {
                let mut store = store(&engine, true)?;
                let theme = pre.instantiate(&mut store)?;
                theme.call_render(&mut store, &page())
            })?,
        );
    }

    // Calls, with budgets on and off. Each sample is a batch, divided by its size.
    for budgets in [true, false] {
        let engine = engine(Allocator::Pooling, budgets)?;
        // Fuel and epochs change the compiled code, so this compiles afresh.
        let component = Component::new(&engine, &bytes)?;
        let pre = ThemePre::new(linker(&engine)?.instantiate_pre(&component)?)?;
        let mut store = store(&engine, budgets)?;
        let theme = pre.instantiate(&mut store)?;
        let probe = theme.ink_theme_probe();
        let tag = if budgets { "fuel+epoch" } else { "no budgets" };
        const BATCH: u32 = 1000;
        show_ns(&format!("{tag}: host->guest call, nothing()"), time(500, || {
            for _ in 0..BATCH {
                probe.call_nothing(&mut store)?;
            }
            Ok(())
        })?, BATCH);
        let empty = time(500, || probe.call_measure(&mut store, 0))?;
        let full = time(500, || probe.call_measure(&mut store, BATCH))?;
        show_ns(&format!("{tag}: guest->host import, text.measure"), full.saturating_sub(empty), BATCH);
        let full = time(200, || probe.call_stroke(&mut store, BATCH))?;
        show_ns(&format!("{tag}: guest->host import, pen.stroke (incl. pen)"), full, BATCH);
        show(&format!("{tag}: render one page (warm instance)"), time(5000, || theme.call_render(&mut store, &page()))?);
    }

    // The pen alone, natively, to separate its cost from the boundary's.
    let native = time(200, || {
        let mut total = 0;
        for i in 0..1000_u32 {
            let to = freehand::Point::new(100.0 + f64::from(i % 7), 0.0);
            total += freehand::mark::line(freehand::Point::new(0.0, 0.0), to, 2.0, "probe").as_str().len();
        }
        Ok(total)
    })?;
    show_ns("native: freehand::mark::line", native, 1000);
    // The underline `render` asks for: the stub measures the title at 336 px.
    let title = time(2000, || {
        Ok(freehand::mark::line(freehand::Point::new(4.0, 6.0), freehand::Point::new(340.0, 6.0), 2.0, "Notes from the margin"))
    })?;
    show("native: the title's underline", title);

    let engine = engine(Allocator::Pooling, true)?;
    let component = Component::new(&engine, &bytes)?;
    let pre = ThemePre::new(linker(&engine)?.instantiate_pre(&component)?)?;
    let mut store = store(&engine, true)?;
    let html = pre.instantiate(&mut store)?.call_render(&mut store, &page())?;
    let fuel = FUEL - store.get_fuel()?;
    let page_path = out.join("page.html");
    std::fs::write(&page_path, &html)?;
    println!("rendered {} ({} bytes, {fuel} fuel)", page_path.display(), html.len());
    Ok(())
}

fn main() -> wasmtime::Result<()> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let wasm = args.next().unwrap_or_else(|| "../out/theme-unknown.wasm".into());
    let out = args.next().unwrap_or_else(|| "../out".into());
    run(&wasm, &out)
}
