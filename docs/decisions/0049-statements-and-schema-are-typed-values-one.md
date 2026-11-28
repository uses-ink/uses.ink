---
number: "0049"
title: "Statements and schema are typed values; one renderer writes the text"
status: accepted
date: 2026-11-26
area: data
supersedes: []
---

## 0049 Statements and schema are typed values; one renderer writes the text

Accepted. No SQL text is written in code, tests or files. A table is an identifier type (an
enum deriving `sea_query::IdenStatic`), a column is one of its variants, a statement is a
`sea_query` builder expression, a row is read back through typed columns (`Row::get(Column)`),
and the migrations are Rust values: `db::Migration` has a number, a name and the statements that
apply it, and `Steps` refuses a number that does not rise. The `db` port crate holds the builders
(pure, no driver) and `db::Renderer`, the one type that turns a statement into SQLite text with
its bound values and checks D1's limit of 100 parameters; the adapters (`crates/port/db/sqlite`, and `crates/port/db/d1`,
planned) run the text and bind the values. The ledger is still `d1_migrations` with `NNNN_name.sql`
names, as wrangler writes it, so a database keeps one history; but the wrangler CLI no longer applies migrations: the
Rust runner does, through `rusqlite` self-hosted and through the D1 binding when hosted. A
`CHECK (x IN (...))` is built from the names the code reads back, so schema and code cannot
drift. `cargo xtask layout` rejects a string literal that starts with a SQL keyword, the
builders' raw escapes (`cust*`, `raw_sql`, `raw_query`) and any `.sql` file outside `spikes/`.
This reverses the "raw SQL" and "wrangler's format" of 0030 and the plain `.sql` folder of the
M3 plan; the shape of the port (statements, atomic batches, no interactive transactions) stands.
**Why:** a query that is a value is checked by the compiler, cannot be built from input, and
renders for any SQLite dialect, so D1 shares the statements. `sea-query` 1.0 is MIT OR Apache-2.0
and with the `derive` feature it adds only proc-macro crates (`sea-query-derive`, `darling`,
`heck`, `syn`, `quote`, `proc-macro2`, `thiserror`). **Reversal:** a hosted adapter that
needs a statement the builders cannot say: add the builder's missing piece to `Renderer`, not a
string to a caller.
