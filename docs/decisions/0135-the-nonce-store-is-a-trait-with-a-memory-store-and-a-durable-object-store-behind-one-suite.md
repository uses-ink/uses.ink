---
number: "0135"
title: "The nonce store is a trait with a memory store and a Durable Object store behind one suite, and the object's ledger refuses requests older than itself"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0135 The nonce store is a trait with a memory store and a Durable Object store behind one suite, and the object's ledger refuses requests older than itself

Built (2026-11-28), A5 of `docs/hosted.md`. Settles the third choice of 0079 as the plan had it: the store is a Durable
Object, and the D1 unique key stays possible because both would satisfy the same suite.

- **`Nonces` is a trait in `crates/server/edge/api`** (`async claim(Claiming) -> Result<Claimed, Error>`), with
  `Remembered` the memory store the self-host keeps. `Signatures` takes the store as a fourth type parameter that
  defaults to `Remembered`, so `Signatures<D, K, O>` and `Signatures::new` read as before; `with_nonces` takes another
  store, and `Failed` gets a third parameter (default `Infallible`) with a `Nonces` variant. `Claimed` gained `Quiet`,
  which `Signatures` answers as `restarting`. The order of the checks is unchanged: skew, boot barrier, key, signature,
  liveness, then the nonce.
- **The suite** is `NonceCase` (5 cases: a replay, a split by key, the end of a window, the bound per key with room
  coming back as claims end, replay before full), public in `edge-api` so a Worker can run it. It takes a `base` time after the store's quiet period and uses a key of its own per case, so a store that keeps
  its state between cases (an object) is not in the way. `Remembered` passes it in `tests/nonces.rs`.
- **`crates/server/edge/object/nonce`** is the pure state machine: a `Ledger` built with its start time and the window,
  `claim(Ask) -> Verdict`. **The boot rule** lives in it: a ledger refuses with `Quiet` any request created before
  `boot + window`, and the object builds its ledger when the object is built, so an eviction or a move restarts the
  quiet period. Nothing is written to the object's storage: the ledger is memory, which the boot rule makes safe, and
  the cost is up to one window of "try again" after an eviction (the open risk the plan names).
- **`crates/server/edge/worker`** is the shell (this is the skeleton A5 needed): `Serving` is the ledger behind a JSON
  wire (`handle(bytes)`) and also a `Nonces` store, so the suite and the boot rule run natively on it; `Remote`
  (`wasm32` only) is the store a Worker uses over a namespace binding. **One object per key**, not per account as
  `docs/hosted.md` section 7 says: a key waiting to be enrolled has no account yet, the ledger is a few kilobytes,
  and the bound is per key anyway. The hop is the same.
- **The class is declared by the deploying Worker**, around `Serving::handle` (`NonceObject` in the rig, and the
  rig needs `wasm-bindgen` as a direct dependency for the macro). A `#[durable_object]` class inside a dependency is
  not reliably linked into the final module, so the shell offers the logic, not the export.

Numbers: native, 118 tests across `edge-api`, `edge-worker` and the ledger pass, including the whole existing API suite
over the generic `Signatures`.
**Not done:** the product Worker that wires `Remote` into `Signatures` (it needs the Worker entry, M4 later), eviction
timing measured on Cloudflare, per-account objects.
**Reversal:** a D1 store is one more `impl Nonces` and the same `NonceCase`.
