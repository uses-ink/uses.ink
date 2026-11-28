# uses-ink-blob-r2

The blob port (`uses-ink-blob`) on a Cloudflare R2 bucket, through workers-rs, for the hosted service
(decision 0131).

It exists only on `wasm32-unknown-unknown`: on any other target the crate is empty and has no
dependencies, so the workspace builds and tests natively. The conformance suite runs on it inside
workerd, with no account, through `cargo xtask worker blob`.

How a blob is stored (decision 0096 measured it, 0131 decides it):

- **Keys** are `<tenant>/<hash>`, both validated types, so no key is built from text an outsider chose.
- **A write hashes as it takes the bytes** and holds at most one part of them. R2 needs a known
  length or a multipart upload, so a blob of at most `Limits::part` bytes (8 MiB by default) is put
  whole once its hash matched, and a longer one is cut into parts as it grows.
- **A hash that does not match aborts the upload** and stores nothing; a failed abort is a typed
  `Error::Abandoned` that names the key, because R2 then keeps the parts until its own expiry.
- **A blob already held** is read for its hash only: nothing is uploaded, and `created` is false.
- **A read verifies as it streams**: the last `chunk` call is `Corrupt` when the bytes do not hash to the key.
- **Listing** pages by `Limits::page` keys and follows R2's cursor; a key that is not a hash is `Error::Stray`.

`Limits::new` refuses a part under R2's least of 5 MiB and a page over 1000 keys.
