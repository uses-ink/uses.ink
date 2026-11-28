---
number: "0131"
title: "The R2 adapter hashes while it takes bytes, puts whole up to one part and cuts a multipart upload past it"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0131 The R2 adapter hashes while it takes bytes, puts whole up to one part and cuts a multipart upload past it

Built (2026-11-28), A1 of `docs/hosted.md`. The gateway shape is the one 0096 measured.

- **`crates/port/blob/r2`** (`uses-ink-blob-r2`) implements `Blob` over an R2 `Bucket` through workers-rs. Like the
  D1 adapter it is empty off `wasm32` and is a workspace member, so the native stages see no code. Keys are
  `<tenant>/<hash>`, both validated types.
- **A write holds one part.** The bytes are hashed with blake3 as they are taken and collected into a buffer. A blob
  of at most `Limits::part` bytes (8 MiB, configurable, at least R2's 5 MiB) is put whole after its hash
  matched, so a mismatch has stored nothing. A longer one starts a multipart upload and sends a part each time the
  buffer passes the part size; the peak is one part plus the chunk in hand. A mismatch aborts the upload and
  answers `Mismatch`. A failed abort is `Error::Abandoned`, with the abort's cause as its source and the reason
  the write was given up, because R2 then keeps the parts until its own expiry.
- **A blob already held is not uploaded.** The adapter asks `head` first. A held blob has its bytes hashed and
  nothing sent, and `created` is false. Two writers of one blob race to the same bytes, so the only cost of the
  race is a flag.
- **A read verifies as it streams.** The body's stream is hashed, and its end is `Corrupt` when the bytes do not
  hash to the key. workers-rs ends a stream that failed with `aborted` as if it finished, so the hash is what
  catches a cut body.
- **Listing** pages by `Limits::page` (1000, the most R2 lists) and follows the cursor; a key under the tenant's
  prefix that is not a hash is `Error::Stray`, and a listing that goes on with no cursor is `Error::Unfinished`.
  Nothing is skipped.
- **The suite grew two cases and a list.** `blob-conformance` has `Case` (7 cases now) like the row port's, with
  `suite!` unchanged for the directory adapter. `round_trips_a_blob_past_one_request` stores 20 MiB in chunks of
  an odd size and reads it back hash for hash; `leaves_nothing_of_a_large_write_that_fails` sends 12 MiB under
  another name and finds nothing held or listed. The Worker rig runs the suite with parts of 8 MiB and pages of
  3 keys, so the listing case crosses pages.
- **The rig is one Worker for every suite.** `spikes/hosted/workers/rig` lists `<suite>/<case>` at
  `GET /__conformance` and runs one at `POST /__conformance/<suite>/<case>`, emptying the store first.
  `cargo xtask worker blob` runs the blob suite, `db` the row suite, `rig` every suite; the stage `worker` runs
  `rig` and `migrate`.

Numbers, local R2 in workerd: the rig is 674,111 B raw and 204,618 B gzip with both adapters; 7 of 7 blob cases
pass; a case takes 22 to 43 ms, except 675 ms for 20 MiB in three parts and 197 ms for 12 MiB aborted. Natively
the directory adapter passes the same 7 cases.

**Not done:** real R2 (the multipart limits and expiry of abandoned parts, 3.2 of the plan), and a delete of a
tenant's blobs in batches (`delete_multiple`), which is the deletion work C3 asks for.
**Reversal:** real R2 refuses the parts the simulator takes: cut at 5 MiB, or hold a tenant's blobs in a Durable
Object's storage.
