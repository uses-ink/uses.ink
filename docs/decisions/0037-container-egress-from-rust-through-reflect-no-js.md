---
number: "0037"
title: "Container egress from Rust through `Reflect`; no JS shim"
status: accepted
date: 2026-11-26
area: hosting
supersedes: []
---

## 0037 Container egress from Rust through `Reflect`; no JS shim

Accepted, spike (`spikes/workers/`). The build Durable Object starts the Container with
`enableInternet: false` and registers outbound intercepts itself: `ctx.exports.default({ props })`
(called with `Reflect::apply`) is the handler, so the worker's own `#[event(fetch)]` is the egress
policy and tells intercepted requests apart by their props. `exec` and the intercepts are called
through `js_sys::Reflect` until workers-rs binds them (#1072; PRs #1078, #1143, #1144). The
handler, not the container, attaches the forge credential. Live logs reach the DO over the
builder's HTTP port or `exec` stdout; the DO streams them and writes the log to R2. **Why:**
DO streaming, R2, the loopback handler and intercept registration all ran from Rust locally, so a
shim and an internet-on v1 are not needed. **Unverified:** the container never started under
`wrangler dev` on native Linux Docker (workers-sdk #15394), so start time, the container-to-DO
stream and enforced egress wait for a host where local containers run, or a hosted preview.
**Reversal:** hosted runs show intercepts do not accept the default-entrypoint loopback.
