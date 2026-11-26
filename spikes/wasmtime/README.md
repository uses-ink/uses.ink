# Spike: wasmtime host, pen and text imports

M0 spike 1 from `docs/roadmap.md`: a Rust theme component imports `pen.stroke` and
`text.measure` and renders a page; measure instantiation and call cost with pooling and `.cwasm`.
Also settles which generator gives the host-side WIT types, and how a guest is built.

**Answer: yes.** The architecture in sections 6 to 8 holds as written, with four corrections
listed under "Against the architecture". Recorded as decision 0035.

## What is here

| Path | What |
|---|---|
| `wit/theme.wit` | `ink:theme@0.1.0`: a `page` record, `render: func(page) -> string`, and a `probe` interface used only for timing |
| `wit/deps/host/host.wit` | `ink:host@0.1.0` stubs: `pen.stroke(start, end, weight, seed) -> string` and `text.measure(font, size, text) -> extent` |
| `theme/` | the guest: `wit-bindgen` 0.63, `unspliced` 0.1.1 for the HTML; measures the title, asks the pen for an underline that wide, renders `<h1>`, the `<svg>` and the paragraphs |
| `host/` | the host and bench: `wasmtime` 51.0.0 with `component::bindgen!`; `pen.stroke` is `freehand::mark::line` from `../../../freehand`; `text.measure` is a **stub** (half an em per char, 1.2 em high) |

Each crate is its own Cargo workspace, so `target/` stays in `theme/target` and `host/target`.
Outputs (components, `.cwasm`, the rendered page) go to `out/`, which is ignored.

## Running it

```sh
cd theme
cargo build --release --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/release/uses_ink_spike_theme.wasm \
  -o ../out/theme-unknown.wasm
cd ../host
cargo build --release
taskset -c 2 ./target/release/uses-ink-spike-host ../out/theme-unknown.wasm ../out
```

## Method

Release builds on this machine: Intel Core Ultra 5 135U, Fedora, Linux 7.2, Rust 1.100.0. Each
row is the median of many timed rounds with `std::time::Instant` (200 to 20,000 rounds, see
`host/src/main.rs`). Per-call rows time batches of 1,000 calls and divide. The bench ran three
times pinned to one P-core (`taskset -c 2`) while another spike was building in parallel; the
table shows the middle run and the range of the three.

Budgets are on (fuel and epoch interruption) unless a row says otherwise, because the
architecture runs every plugin call under both. The pooling allocator caps memory at 64 MiB per
instance.

Definitions:

- **compile**: `Component::new` on the component bytes, Cranelift.
- **cold**: a fresh `Engine`, `Component::deserialize_file` on the `.cwasm`, a fresh `Linker`,
  instantiate, one `render`. What the first page of a build pays.
- **instantiate**: a fresh `Store` (fuel and deadline set) and `ThemePre::instantiate`, the way
  the engine would give each page its own instance.
- **import call**: `text.measure` called from inside the guest, minus the empty loop.

## Numbers

| What | Median | Range of 3 runs |
|---|---|---|
| compile, Cranelift, one core | 49.2 ms | 48.1 to 61.8 ms |
| compile, Cranelift, all cores (unpinned) | 12 to 21 ms | |
| `.cwasm` load, `deserialize` from bytes | 85.5 us | 83.1 to 92.1 us |
| `.cwasm` load, `deserialize_file` (mmap) | 57.6 us | 55.3 to 59.9 us |
| cold: engine, `.cwasm` file, link, instantiate, render | 205 us | 193 to 209 us |
| instantiate, on-demand allocator, `InstancePre` | 8.0 us | 7.4 to 8.2 us |
| instantiate, on-demand allocator, `linker.instantiate` | 7.3 us | 7.1 to 8.1 us |
| instantiate, pooling, `InstancePre` | **5.8 us** | 5.5 to 6.4 us |
| instantiate, pooling, 1 MiB `linear_memory_keep_resident` | 20.5 us | 20.0 to 21.0 us |
| instantiate and render one page, pooling | 105 us | 104 to 108 us |
| render one page, warm instance, fuel and epoch | 87.4 us | 84.6 to 88.1 us |
| render one page, warm instance, no budgets | 81.2 us | 77.2 to 81.9 us |
| host to guest call, `nothing()`, fuel and epoch | 60 ns | 60 to 63 ns |
| host to guest call, `nothing()`, no budgets | 58 ns | 56 to 67 ns |
| guest to host import, `text.measure`, fuel and epoch | **153 ns** | 144 to 157 ns |
| guest to host import, `text.measure`, no budgets | 150 ns | 147 to 161 ns |
| guest to host import, `pen.stroke` (100 px line, pen included) | 37.0 us | 36.2 to 40.7 us |
| `freehand::mark::line`, native, same line | 35.3 us | 34.3 to 35.5 us |
| `freehand::mark::line`, native, the page's 336 px title underline | 69 us | one later run |
| fuel for one render | 127,028 | |

| Size | Bytes | gzip -9 |
|---|---|---|
| guest core module (`wasm32-unknown-unknown`, `opt-level = "s"`, LTO, stripped) | 25,461 | |
| guest component (after `wasm-tools component new`) | **27,396** | 11,426 |
| guest component built for `wasm32-wasip2` instead | 57,605 | 22,119 |
| `.cwasm` | 118,712 | 45,580 |
| host binary (wasmtime with Cranelift and pooling) | 20.2 MB | 14.5 MB stripped |

| Build | Time |
|---|---|
| guest, clean | 10.8 s |
| guest, one file touched | 0.4 s |
| `wasm-tools component new` | under 10 ms |
| host, clean (wasmtime and Cranelift, 14 threads, machine shared) | 112 s |
| host, one file touched | 1.0 s |

### What the numbers mean

- The boundary is cheap next to the work. A page pays about 6 us to instantiate and 150 ns per
  import; a 1,000-page site with an instance per page pays about 6 ms for instances. The page
  itself is dominated by the pen: the 336 px title underline alone is 69 us natively, about 80%
  of the 87 us render. The HTML from `unspliced` and the boundary share the rest.
- `pen.stroke` through the boundary costs what the pen costs natively plus 1 to 2 us, mostly
  copying back about 4 KB of path text. Keeping the pen host-side (0025) costs nothing measurable.
- Fuel and epochs add about 7% to a render and almost nothing to a call. Leave them on.
- Pooling beats on-demand by about 2 us per instance (5.8 against 8.0). Worth it for the memory
  cap more than for speed. `linear_memory_keep_resident` made instantiation 3.5 times slower at
  1 MiB: the kept pages are zeroed by `memset` on reuse. Leave it at 0.
- A fresh instance's first call costs about 12 to 15 us more than a warm one (page faults in the new
  linear memory). Reusing an instance across pages would save that, but would let one page see
  another's state; an instance per page is the right default at this price.
- `.cwasm` turns a 12 to 50 ms compile into a 58 us load. Cache it.

## Generators

**Host: `wasmtime::component::bindgen!`.** It is the only maintained generator for wasmtime
hosts and its output is clean: each WIT record is a plain struct with `pub` fields
(`pen::Point { x, y }`), each interface a `Host` trait with plain methods
(`fn stroke(&mut self, start: Point, end: Point, weight: f64, seed: String) -> String`), and the
world gives a typed `ThemePre` and `call_render`. Linking is
`Theme::add_to_linker::<_, HasSelf<_>>(&mut linker, |h| h)`. Errors from host functions need the
`trappable` option or `Result` in WIT; the spike's imports cannot fail. A types-only interface
(`ink:theme/page`) still needs an empty `impl page::Host for Host {}`.

**Guest: `wit-bindgen` 0.63 built for `wasm32-unknown-unknown`, then `wasm-tools component new`.**
That component imports exactly `ink:host/pen`, `ink:host/text` and `ink:theme/page`. The same
crate built with `cargo build --target wasm32-wasip2` is twice the size and imports 14 WASI
interfaces from std (`wasi:clocks/monotonic-clock`, `wasi:cli/environment`, `wasi:cli/stdin`,
`wasi:cli/exit`, `wasi:io/*`, terminals), which section 6 forbids. The host refuses it at link
time: `component imports instance wasi:io/poll@0.2.12, but a matching implementation was not
found in the linker`. `cargo-component` is not needed (last release 0.21.1, March 2025). In the
repo, `xtask` should call the `wit-component` crate (0.263, the version wasmtime 51 uses) instead
of the `wasm-tools` CLI.

## Against the architecture

1. **`Component::deserialize` is `unsafe`.** The workspace sets `unsafe_code = "forbid"`, and
   `forbid` cannot be lowered by an `allow` inside a crate. `host/wasmtime` needs its own lint
   table (`deny` instead of `forbid`, everything else inherited by hand) and one `unsafe` block
   with its reason: the `.cwasm` was produced by this binary and is read from a digest-checked
   cache.
2. **The `.cwasm` key is more than the plugin version.** Compiled code depends on the wasmtime
   version, the engine's settings (fuel and epochs change the code) and the CPU features. Key the
   cache by plugin digest plus `Engine::precompile_compatibility_hash`. The allocator is not part
   of it: a `.cwasm` from an on-demand engine loaded into a pooling engine.
3. **Authorities cannot use the host's generated types.** `bindgen!` types derive wasmtime's
   `ComponentType`, `Lift` and `Lower` and live in the host crate. A Worker that cannot host
   wasmtime cannot depend on them. "Native crates against the WIT-generated types" (0007, section
   6) needs a plain-Rust types crate that both sides use, with `From` conversions at the wasmtime
   boundary. Not built in this spike.
4. **Guests target `wasm32-unknown-unknown`, not `wasm32-wasip2`**, for the reason above.
   `libraries.md` already says so; nothing else should suggest wasip2 for plugins.

## Gotchas

- `from` is a WIT keyword: `stroke: func(from: point, ...)` does not parse. The stub uses `start`
  and `end`.
- `wit-bindgen` 0.63 refuses a world whose imports live in `deps/` unless each is mapped with
  `with` or `generate_all` is set. A real repo maps `ink:host` to one shared generated module.
- `bindgen!` and `wit_bindgen::generate!` both put the world's `use`d types at the top of the
  module, so `use ink::theme::page::Page` beside them is a duplicate.
- `unspliced` renders no floats, so the theme rounds geometry to whole pixels before writing
  `width` and `height`. That is fine for SVG boxes; path data arrives as text from the pen.
- `pen.stroke` returns about 4 KB of path text for a 336 px line. If pages carry many marks, a
  typed path (`list<point>`) or a handle the host writes into the output would save the copy;
  not needed at these numbers.
- The machine was shared with another spike's build during the runs. Compile time moves the most
  (48 to 62 ms pinned); instantiation and call rows stayed within about 10%.
