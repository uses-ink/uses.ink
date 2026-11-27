---
number: "0035"
title: "Plugins run on wasmtime as written; guests target `wasm32-unknown-unknown`"
status: accepted
date: 2026-11-26
area: plugins
supersedes: []
---

## 0035 Plugins run on wasmtime as written; guests target `wasm32-unknown-unknown`

Accepted. The M0 spike (`spikes/wasmtime/`, removed once `host/wasmtime` replaced it; its README
is in the history) ran a Rust theme component importing
`pen.stroke` (the real `freehand`) and `text.measure` on wasmtime 51: a pooled instance costs
about 6 us, a guest-to-host import about 150 ns, a `.cwasm` load 58 us against a 12 to 50 ms
Cranelift compile, and fuel with epochs about 7% of a render. The pen, not the boundary, is the
cost of a page. Host types come from `wasmtime::component::bindgen!`; guests use `wit-bindgen`
0.63 built for `wasm32-unknown-unknown` and wrapped by `wit-component`, because a `wasm32-wasip2`
build imports 14 WASI interfaces (clocks, environment, stdin) that plugins may not have.
`cargo-component` is not used. Follow-ups: `host/wasmtime` carries one reasoned `unsafe` for
`Component::deserialize`, so `xtask check`'s allowlist (0034) gains that file when `host/wasmtime` lands; the `.cwasm` cache is
keyed by plugin digest plus `Engine::precompile_compatibility_hash`; authorities need a plain
types crate, since the generated types derive wasmtime traits. **Why:** the measured overhead is
small next to the work a page does. **Reversal:** a real theme where instantiation or imports
exceed 20% of render time.
