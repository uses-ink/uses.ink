# uses-ink-db-d1

The row port (`uses-ink-db`) on Cloudflare D1, through workers-rs, for the hosted service.

It exists only on `wasm32-unknown-unknown`: on any other target the crate is empty and has no
dependencies, so the workspace builds and tests natively. The conformance suite runs on it inside
workerd, with no account, through `cargo xtask worker db`.

What D1 does to the port (decision 0097):

- **Integers** are JavaScript numbers across the boundary, exact up to 2^53 - 1 (`Value::SAFE`).
  The renderer refuses a wider integer for every adapter, and this adapter refuses a number it
  cannot tell from a wider one. A wider `u64` is kept as text.
- **Reals and integers** share one JavaScript type, so a whole number reads back as an integer;
  `f64` columns accept it.
- **Blobs** are bound as `Uint8Array` and come back as arrays of bytes.
- **Constraints** are told apart by the `SQLITE_CONSTRAINT_*` code in D1's error text.
- **Batches** are atomic. A statement that reads names its columns from the result's objects, so
  two columns with one name collapse into one inside a batch; `run` and `rows` keep both.
- **Parameters** are limited to 100 per statement, checked before the call.
