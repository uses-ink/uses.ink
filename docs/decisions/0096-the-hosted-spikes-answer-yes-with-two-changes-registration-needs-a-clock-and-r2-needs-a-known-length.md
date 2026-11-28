---
number: "0096"
title: "The hosted spikes answer yes with three changes: registration needs a clock patch, D1 carries integers to 2^53 only, and R2 needs a known length"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0096 The hosted spikes answer yes with three changes: registration needs a clock patch, D1 carries integers to 2^53 only, and R2 needs a known length

Built (2026-11-28). The H0b spikes of `docs/hosted.md` section 5.0 ran in `workerd` through
`cargo xtask worker <api|store>` (stage `worker`, farm job `worker`, x86 hosts with node), with no
account. They are `spikes/hosted/{api,store}`, outside the cargo workspace; npm is pinned in
`spikes/hosted/package.json` and its lockfile (wrangler 4.168.0, which brings miniflare and workerd
1.20261127), installed under `target/hosted/`; Rust is `worker` 0.9.0 and `worker-build` 0.9.0.

**(a) Worker size and start: yes.** The Worker links `control`, `edge`, the ports, `sig`,
`webauthn-rs-core` 0.6.2 and the real `webauthn.rs` adapter of `edge/api`.

| | raw | gzip -9 |
|---|---|---|
| `index_bg.wasm`, `worker-build --release` | 1,164,218 B (1.11 MiB) | 386,189 B (0.37 MiB) |
| `wrangler deploy --dry-run` upload | 1170.17 KiB | 391.85 KiB |

The limit is 64 MiB uncompressed for Free and Paid (Cloudflare's docs, read 2026-11-28); the
older 3 MiB and 10 MiB gzip limits would also hold it. The first request answers in 43 ms and the
second in 6 ms, with the module compiled. A passkey ceremony runs in `workerd`: a software ES256
authenticator signs in (14 ms for the login and four refusals: tampered signature, wrong origin,
other challenge) and registers (4 ms). This closes the open "run in workerd" of 0068.

Three findings:

- **Registration traps as shipped.** `webauthn-rs-core` calls `SystemTime::now()`, which panics on
  `wasm32-unknown-unknown` ("time not implemented"), so `finish_registration` stops the Worker; the
  sign-in does not read the clock. A copy of 0.6.2 under `spikes/hosted/vendor/` that reads
  `Date.now()` there (one `Clock` type, two call sites) passes. W2 carries it as a `[patch]` of the
  workspace until upstream takes the same change; the alternative is to verify registrations in a
  native service.
- **`crates/server/edge/api` does not build for wasm.** tokio's `net` feature reaches it through
  `uses-ink-clock` (the `System` clock) and `uses-ink-edge-folder`, and mio refuses the target. The
  spike compiles `webauthn.rs` by path instead. W2 splits those two crates so the API sits on
  the ports alone.
- **The bundle is small.** The plan's "two Workers if one is too big" is not needed at this size.

**(b) D1 value fidelity: yes, with one rule.** Local D1, through workers-rs 0.9:

- Integers cross as JavaScript numbers: `-(2^53-1)` to `2^53-1` are exact. `2^53+1` as a number
  is stored as `2^53`. A `BigInt` is refused (`D1_TYPE_ERROR`). `i64::MAX` as text in an
  `INTEGER` column is stored exactly and read back as `9223372036854776000`. `u64::MAX` as text in
  an `INTEGER` column becomes a `REAL`. As text in a `TEXT` column it is exact.
- So **the port carries integers to 2^53 - 1 (`Value::SAFE`), refused at render time for every
  adapter** (0097), and a `u64` above that is stored as text.
- Blobs bind from a `Uint8Array` (an `ArrayBuffer` or an array of numbers also work) and come back
  as an array of numbers; a 1 MiB round trip takes 146 to 165 ms; the empty blob is `[]`; null is null.
- Text with a NUL and an emoji is exact. `2.0` bound to an untyped column reads as a number, so
  D1 cannot tell a whole real from an integer.
- Bound parameters: 100 run; 101, 500 and 1000 fail with `variable number must be between ?1
  and ?100`. The port's limit of 100 is D1's.
- Errors carry `SQLITE_CONSTRAINT_UNIQUE`, `_PRIMARYKEY`, `_FOREIGNKEY`, `_NOTNULL` and `_CHECK`
  in the cause text, which is how the adapter tells constraints apart. An empty batch fails ("No
  SQL statements detected"); a failing batch leaves nothing; every statement of a batch reports its
  own `changes`.

**(c) R2 streaming put: yes through the gateway, no directly.**

- `bucket.put(key, request.body)` with a chunked body (no length) fails: `Provided readable stream
  must have a known length`. With a `Content-Length` it works (1 MiB).
- The gateway in `spikes/hosted/store` hashes with blake3 as chunks arrive, buffers up to 8 MiB, and
  past that cuts 8 MiB parts into a multipart upload (the minimum part is 5 MiB). 1 KiB: 5 ms; 3 MiB
  in one put: 65 to 75 ms; 20 MiB in 3 parts: 476 ms; the peak buffer is 8 MiB (the isolate has
  128 MB). It completes on a hash match; on a mismatch it aborts, answers 409 and stores nothing
  (12 MiB, 237 ms; the object then answers 404). Unknown length therefore costs at most one
  8 MiB buffer.

**(d) The container start: skipped, it needs the account.** What it needs: a Cloudflare account on
the Workers Paid plan with containers enabled, a `containers` entry in `wrangler.toml` and an image
pushed by `wrangler deploy`, Docker on the deploying host, and the Debian VM of #15394 for a
local `wrangler dev`. The build Durable Object's start time and the egress handler (0037) are
measured there.

**Not covered by a simulator** (`docs/hosted.md` section 3.2): real D1's limits and latency, R2's
real multipart behaviour and the isolate's CPU time. They are checked once in the account.

**Why:** these answer the two reversal conditions of 0079. **Reversal:** the real D1 differs from
the local one in the integer boundary: keep rows in a Durable Object's SQLite behind the same port.
