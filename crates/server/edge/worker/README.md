# uses-ink-edge-worker

The Worker shell of the edge. For now it holds the nonce store as a Durable Object (decision 0135).

- `Serving` is the object's logic: a `Ledger` (`uses-ink-edge-object-nonce`) behind a JSON wire. `handle(bytes)` is
  all the object's `fetch` does. It is also an `edge_api::Nonces` store, so the suite (`edge_api::NonceCase`) runs on it natively.
- `Remote` (`wasm32` only) is the store a Worker uses: one object per key, named by the key, reached through a
  namespace binding.
- The object's `#[durable_object]` class is declared in the deploying Worker, around `Serving::handle` (the rig
  has it as `NonceObject`); a class in a dependency is not reliably linked into the Worker's module.
- Boot rule: `Serving::new(boot, window)` refuses a request created before `boot + window` with `Quiet`. The
  object is built at the time its class is, so an eviction restarts the quiet period.
