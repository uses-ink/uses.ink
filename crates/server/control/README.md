# uses-ink-control

The control plane's core: accounts, sites and handles, deploys with their rollback pointers, build
jobs, credentials and each site's event log. Every use case is a method of `Control`, generic over
the ports it runs on, so the same code runs on SQLite and a directory at home and on the hosted
adapters later.

## What it may use

The ports and the model, and nothing that is an adapter in production: `uses-ink-blob`,
`uses-ink-build`, `uses-ink-db`, `uses-ink-mail`, `uses-ink-queue`, `uses-ink-secret`,
`uses-ink-sig`, `uses-ink-model` and `uses-ink-logging`, plus its own three crates below. The SQLite,
directory and local-key adapters are for its tests only (dev-dependencies). It reads no clock and
no random numbers: the caller draws every id and says what time it is. Only atomic batches and
single-statement compare-and-set are used (decision 0030), so what must hold is in a statement's
`WHERE`. The full list is in [the crate index](../../../docs/crates.md).

## Where things are

The control plane is four crates in one family:

- `crates/server/control/rows` is the floor the tables share: typed ids, column kinds and row
  readers.
- `crates/server/control/signin` holds signing in (decision 0066): one-time email links,
  sessions and the first-run admin link.
- `crates/server/control/keys` holds keys for machines (decision 0068): the rights of a
  credential, the device and deploy keys an account holds, and enrolling them.
- `crates/server/control/src` is the domain and the use cases. `Control` is in
  `crates/server/control/src/control.rs`, the use cases are one type each under
  `crates/server/control/src/case/`, and the passkey ceremony is in
  `crates/server/control/src/passkeys/`.

Rows are made by typed migration values (`Domain` in `crates/server/control/src/migration.rs`);
there is no SQL text. Errors are typed and keep their sources: `Error`, `Refusal`, `Unreadable`.

## How to test it

`crates/server/control/tests/control/` runs the use cases over the real SQLite and directory
adapters in memory, one file per concern. `world.rs` builds the world a test starts from. On the
farm:

    farm cargo nextest run -p uses-ink-control -p uses-ink-control-rows -p uses-ink-control-signin -p uses-ink-control-keys
