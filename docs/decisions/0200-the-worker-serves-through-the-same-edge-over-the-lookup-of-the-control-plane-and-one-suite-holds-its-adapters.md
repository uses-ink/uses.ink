---
number: "0200"
title: "The Worker serves through the same edge over the lookup of the control plane, and one suite holds its adapters"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0200 The Worker serves through the same edge over the lookup of the control plane, and one suite holds its adapters

Built (2026-11-28), W1 of `docs/hosted.md`.

- **The edge is untouched.** 0051 reserved a local flavour of `Sites` and `Blobs` behind a blanket impl. It was not
  built: the `Send` leak through a return-position `impl Future` into `edge/axum` is the risk, and a Worker has one
  thread. The adapters in `crates/server/edge/worker` hold what they read through in a `SendWrapper` and wrap each future
  they return in one, so the traits are implemented as they are. The wrapper panics if it is used from another thread,
  which a Worker's isolate never does. Reversal: the twin of 0051, if a second runtime with real threads and a non-`Send`
  store appears.
- **The lookup is one value.** `edge-control`'s `Found` is what `Back` did apart from placing files: host to handle,
  preview or proved custom domain, the pointer with its ten second time to live, the manifest read back and kept by
  deploy id, and the suspension notice. `Back` holds a `Found` and adds the channel and `Placed`. The crate has a
  `files` feature (default, as 0130 did for `edge-folder`), so a Worker builds `Found` with no channel and no files.
  `Sited<P, K>` is `Sites` over it, with the tenant as the scope.
- **`Stored<B>` is `Blobs` over any blob store of the control plane.** A blob is `stat`ted when it is opened and read
  from its start when it is streamed, so R2's verifying read verifies a whole file. A span skips the bytes before it:
  the cost of a range is the bytes before it, and every byte served is one the hash vouched for. A range that stops short
  of the end is not verified past its own bytes. R2's native ranged get would be cheaper and unverified, as the folder
  adapter's reads are; this is the choice to push back on.
- **`edge-conformance` holds nine cases** (`Case`, `Harness`, `suite!`) for `Sites` and `Blobs`: a host resolves, a blob
  opens with its length, a whole blob and a span read back, a blob is read in its own site's scope only, a page is
  served and a missing route is a 404, a range is a 206, a deploy of 130 files is read whole (past D1's 100 bound
  parameters), a suspended site is a 403 with no bytes, a custom domain serves its site. A case an adapter cannot offer
  is `Outcome::NotOffered` and a runner says so: the folder adapter offers neither suspensions nor domains, and its test
  pins that. The folder adapter, and `Sited` over SQLite and a directory, pass the suite natively; `Sited` over D1 and
  `Stored` over R2 pass it in workerd (`cargo xtask worker edge`, in the rig).
- **`Serve`** (wasm32) turns a Worker's request into the edge's and the answer into a Worker's response, streaming a
  blob. `Platform` is the Worker's `Clock` and `Draw`: `Date.now()`, `setTimeout` and `crypto.getRandomValues`, since
  `clock::System` reads the OS clock and the tokio timer. What the platform rewrites cannot reach the edge: the URL is
  parsed and its dot segments are resolved, and a header repeated is one field, so the refusals for `..`, a doubled
  `Host` and a doubled `Range` never fire on a Worker. The edge's other checks apply.

**Not done:** a ranged R2 read, the Cache API in front of the pointer (the lookup keeps its own for ten seconds in
the isolate), and `Fronts` and `Files` (the `ink serve` adapters) in the suite.
**Reversal:** as above; and if `SendWrapper` ever panics in a Worker, the twin is the answer.
