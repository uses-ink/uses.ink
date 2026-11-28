---
number: "0097"
title: "The db conformance suite is a list of async cases, and the D1 adapter passes all of them in workerd"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0097 The db conformance suite is a list of async cases, and the D1 adapter passes all of them in workerd

Built (2026-11-28), H1 of `docs/hosted.md`.

- **`db-conformance` has `Case`.** One enum lists the 14 cases with a name each, `Case::ALL`,
  `Case::named` and `async fn run<D: Db>(self, &D)`. `suite!` is unchanged for adapters: it expands
  to one native test per case that blocks on `Case::run` with `pollster`. A Worker cannot block, so
  the rig awaits the same function. The SQLite adapter also runs the whole list in one test.
- **`crates/port/db/d1`** (`uses-ink-db-d1`) implements `Db` over a `D1Database` through workers-rs. It
  compiles only for `wasm32` (its dependencies are in a `cfg(target_arch)` table, so natively it is an
  empty crate and the workspace builds). `Ledger` needs no code: the migration runner is generic
  over `Db` and writes wrangler's `d1_migrations`. `layout.toml` ENFORCED, MAY_USE (`db`, `logging`),
  its README, `docs/crates.md`, and the wasm clippy list of the `lint` stage cover it.
- **The rig** `spikes/hosted/workers/rig` is a Worker with `GET /__conformance` (the names) and `POST
  /__conformance/<name>` (drops the tables, runs one case). `cargo xtask worker db`, which the
  stage `worker` runs, calls each: 14 of 14 pass in workerd; the rig is 518,901 B raw and
  157,767 B gzip; a case takes 40 to 490 ms. Natively 28 tests of the db, sqlite and queue-rows
  crates pass.

What D1 changed in the port (0096 has the measurements):

- **Integers to 2^53 - 1.** `Renderer` refuses a wider `i64` with `Unsupported::Unsafe` for every
  adapter, so none stores what another reads back changed. `Values` now round-trips `+-Value::SAFE`;
  `Limit::unsupported` also refuses `SAFE + 1`. The adapter reads a whole number past 2^53 - 1 as a
  typed `Unread::Rounded`, because D1 cannot tell a rounded integer from a real.
- **`f64` columns accept an integer** within 2^53, since D1 cannot tell `2.0` from `2`.
- **Reads** use `raw` with column names for a `select` (one name per column, duplicates kept) and
  `run` for the rest. Inside a batch rows come as objects, so two columns of one name collapse.
- **Constraints** come from the `SQLITE_CONSTRAINT_*` token in D1's cause text, kept whole as the source.
  Nothing is retried or defaulted: a missing `changes` is `Error::Meta`.

**Not done:** the other hosted adapters (R2, queue wake-up, secrets, mail) and real D1.
**Reversal:** as 0096.
