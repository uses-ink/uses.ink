---
number: "0041"
title: "Components as built: values across, one shared mirror"
status: accepted
date: 2026-11-26
area: plugins
supersedes: []
---

## 0041 Components as built: values across, one shared mirror

Accepted. `ink:format-markdown` and `ink:theme-quiet` run as components through `crates/runtime/host/wasmtime`,
and `ink build --check` builds every CLI site fixture the same, byte for byte, natively and
through them. What 0035 and architecture.md sections 6 and 7 did not settle, as built:

- **The WIT lives at `wit/<package>/`**: `ink:model` (the model's types), `ink:host`
  (capabilities, and a `plugin` world every binding is generated from once), `ink:format`,
  `ink:theme`. Each slot's world maps its imports onto the `plugin` bindings with `with`, so one
  set of types exists per side. Generators get several paths in dependency order; no `deps/`
  copies.
- **The model crosses as values.** A document is flattened into two lists (blocks, inlines) whose
  nodes name their children by place, children first, each used once, and is rebuilt in one
  pass without recursion, so hostile input cannot make a cycle, share a node or overflow the
  host's stack. A problem or announcement crosses as its code and its fields in catalog order
  (`model::coded`), so a new catalog entry changes no WIT. A diagnostic's cause chain is a list.
  Each asset carries the `picture` the engine prepared (0040: size, copies, placeholder), so a
  theme component writes `<picture>` exactly as the native one does.
- **A theme renders one site per instance.** Its `pages` export is a `renderer` resource built
  from the base address and the site; each page is then one call. The adapter keeps the session
  while the engine passes an equal site and starts a new instance otherwise or after a failed
  call. A format call gets an instance of its own (about 6 us, 0035). Architecture's host-owned
  `site` resource with lazy reads is not built: the native core takes a whole `Site`.
- **Conversions are written once for both sides.** `wit/mirror/` holds them as `From` and
  `TryFrom` impls naming the generated types through `super::wit`; `crates/runtime/host/wasmtime` and
  `crates/runtime/plugin/sdk` each include its parts over their own bindings, since `bindgen!` and
  `wit-bindgen` write the same shapes but no common type.
- **Memory is limited by a `ResourceLimiter`**, not the pooling allocator, so a refused growth is
  told apart from other traps (`plugin.budget.memory`). Fuel and an epoch ticker (5 ms) give
  `plugin.budget.fuel` and `plugin.budget.time`; a trap is `plugin.call.trapped`, a value the
  model refuses `plugin.value.malformed`. Every one is a diagnostic and the next call is served.
- **`text.measure` refuses every font** (`font-unknown`): the measurer is not built.
- **Components are read from a folder** (`--components`, written by `cargo xtask component` to
  `target/plugin/`), not bundled in `ink`; their `.cwasm` cache is its `cache/` folder, each file
  checked by its BLAKE3 before it is loaded, a file that does not check compiled again.
- **The model, the SDK and `wit/` are MIT OR Apache-2.0** (0026). The mirror converts to
  nearly all of the model: a site names decision codes, whose evidence holds a snapshot's
  provenance, and an asset's picture holds a manifest's media type, so a cut of the types that
  cross would have left `model` an empty shell around `Snapshot` and `Manifest`. The model is
  plain values with only permissive dependencies, so it is relicensed whole instead of cut.
  `Record`, `Said` and a theme's `File` (the WIT's `ink:model/file`) moved from the engine to
  the model, which the engine re-exports, so the SDK depends on `freehand` and `model` only. `cargo xtask layout` refuses a permissive crate
  with a normal or build dependency on an AGPL crate of ours (`Licence`, `Problem::Copyleft`).
  The engine's plugin traits and their types (`Format`, `Theme`, `Html`, `BaseUrl`) stay AGPL: a
  component needs only the SDK, and a third-party native core written against them is a later
  decision.

Measured (release, this machine, `crates/runtime/host/wasmtime/tests/measure.rs`): the markdown component is
1,369,744 bytes (464 KB gzipped, a 4.5 MB `.cwasm`), the quiet theme 657,499 (227 KB, 2.2 MB);
a Cranelift compile takes 402 ms and 244 ms on all cores, a cached load (a new engine, the digest
checked, `deserialize`) 10 ms and 5 ms. The blog fixture, interpreted and rendered, takes 0.31 ms
natively and 2.8 ms through the components: on five files the boundary is all of the cost, and
0007's reversal (formats over 20% of build time) is to be judged on a real site, not on this.
`ink build --check` of that fixture takes 0.77 s on a cold cache and 66 ms warm, in a debug build.

**Why:** the native cores stay as they are, the boundary is written once, and the check builds
the same bytes both ways. **Reversal:** a site large enough that sending it to each theme
instance shows in build time; then `site` becomes a host resource read lazily.
