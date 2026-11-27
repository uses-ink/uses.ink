---
number: "0034"
title: "The pen in the browser is plain wasm and one small loader"
status: accepted
date: 2026-11-26
area: brand
supersedes: []
---

## 0034 The pen in the browser is plain wasm and one small loader

Accepted. `freehand-web` exports the marks (`line`, `underline`, `frame`, `fill`, `patch`,
`drawn`) as plain wasm functions: numbers and a name in, SVG path data out, through two buffers
the module owns. `freehand-web/freehand.js` (about 40 lines) loads it; the page measures elements
and inserts the paths. The mock's `pen.js` now draws through it, and `pen-core.js`, `gen-*.mjs`
and the vendored perfect-freehand are gone. The workspace denies `unsafe_code` instead of
forbidding it, because a wasm export's name needs `#[unsafe(no_mangle)]`; `xtask check` allows
unsafe code in `freehand-web/src/lib.rs` and nowhere else. **Why:** `wasm-bindgen` generates JS
glue on every build, against 0032; marks pass a few numbers, so the protocol is small enough to
own. **Reversal:** if the dashboard needs many more exports, revisit `wasm-bindgen`.
