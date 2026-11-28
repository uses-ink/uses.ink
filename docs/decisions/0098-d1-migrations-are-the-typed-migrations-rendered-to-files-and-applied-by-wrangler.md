---
number: "0098"
title: "D1 migrations are the typed migrations rendered to files and applied by wrangler, and the schema is the same as SQLite's"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0098 D1 migrations are the typed migrations rendered to files and applied by wrangler, and the schema is the same as SQLite's

Built (2026-11-28), H1b of `docs/hosted.md`.

- **`Renderer::inline`** writes a statement as one line with its values as escaped literals
  (`'it''s'`, `x'00FF'`), after the checks of `render`. It is in `render.rs`, the one file that
  writes SQL (0049). **`Steps::scripts`** returns each migration as a `Script`, named as wrangler
  names a file (`0001_first.sql`, the name the port's ledger already records). The ledger row is
  not in the file: `wrangler d1 migrations apply` writes it.
- **The files are generated, never committed.** The layout rule forbids `.sql` in the tree, so
  `cargo run -p uses-ink-db-sqlite --features scripts --bin scripts -- <folder>` writes them to
  `target/hosted/migrate/out/migrations`, with the two queries and the schema SQLite ends with.
  `spikes/hosted/workers/migrate/wrangler.toml` points `migrations_dir` there.
- **`cargo xtask worker migrate`** (in the stage `worker`) applies them with `wrangler d1
  migrations apply DB --local`, reads `sqlite_master` and the ledger back with `wrangler d1 execute
  --local --json`, and compares with SQLite's, line by line. Result: 2 objects and 2 ledger rows,
  the same definitions byte for byte (the conformance suite's two migrations).
- **The real schema is next.** `control`'s own migrations are another crate's (the wiring work);
  when its `Steps` is public, the `scripts` bin takes it in place of `Migrations::steps()`.

**Not done:** applying to a real D1 (`--remote`), and comparing row data.
