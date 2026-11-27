---
number: "0051"
title: "The edge answers from two ports and refuses what it cannot name; the adapter owns the times"
status: built
date: 2026-11-27
area: edge
supersedes: []
---

## 0051 The edge answers from two ports and refuses what it cannot name; the adapter owns the times

Built (2026-11-27) as the serving half of M3: `edge`, `edge/axum`, `edge/folder` and `ink serve`.
The owner has not read it yet; the choices below are the ones to push back on.

- **Two ports, no host types.** `Sites` (host to the deploy's manifest) and `Blobs` (a hash, read
  in the scope `Sites` resolved: a tenant, a folder) are async traits whose futures are `Send`.
  A blob is opened first, so its length is known before a byte is read (ranges, `content-length`,
  the size budget), then read as a stream of chunks. The core owns no socket, file, clock or
  runtime; `Edge::respond` takes bytes and returns a typed `Answer`. workers-rs futures are not
  `Send`, so the planned `edge/worker` wraps these traits in its own twin; the logic is the same code.
- **The edge parses the request, not the framework.** The adapter passes the method, the target
  and the header lines exactly as received, and the edge refuses with a typed `Refusal` that
  carries its data (`Refusal` has the data and a status; its text is its `Display`, once). The
  rule is to refuse what could name two things, never to repair it: `..` and `.` segments
  (escaped or not), `%2f` and `%5c`, a `%` that survives one decode (`%252e`), NUL and other
  controls, `//` (so `//evil.example/` is never a redirect target), a raw non-ASCII byte, a
  fragment, a target over the limit, an absolute-form target for another host than `Host`, two
  `Host` headers, a `Host` that is not a DNS name or address (so it can be joined to a
  folder; it is read in any case and lowered once, 0058), a `Content-Length` above zero or any `Transfer-Encoding` on a read, and the same
  header twice where two values would be two requests (`Range`, `If-Range`, `If-None-Match`,
  `Accept-Encoding`). A trailing `/` is dropped without a redirect; a path that is no route
  (`/Tea`, `/tea.html`) is a 404, not a refusal, because it names nothing. No NFC normalisation:
  routes are what the engine wrote.
- **Headers.** A manifest entry's headers are sent as they are (the engine's page and file
  policy). The edge fills the strict file set (`default-src 'none'; frame-ancestors 'none';
  sandbox`, `cross-origin-resource-policy: same-origin`, `nosniff`) into any response lacking
  them: every refusal, 304, 301, 416, 500, `/healthz` and a manifest from another producer. That
  set is a copy of the engine's `Policy::FILE`, which stays `pub(crate)` so `edge` does not
  depend on `engine`; `cli/tests/serve.rs` builds a site and fails when the copy drifts.
  `Strict-Transport-Security` is sent only to a request that came over TLS, a setting of the
  adapter (`Scheme`), so a plain-HTTP self-host never tells a browser to pin it.
  `content-type` comes from the entry's media type, `etag` from the blob's hash (`"<hash>"`, or
  `"<hash>-br"` for a variant), `cache-control` from the entry or `public, max-age=0,
  must-revalidate`; refusals are `no-store`.
- **Conditional and range.** `If-None-Match` is compared weakly, takes `*` and lists (at most
  `Limits::tags`), and gives 304 before anything is opened. `Range` is served for `GET` only,
  for one `bytes` span (`a-b`, `a-`, `-n`) as 206 with `Content-Range`; a span starting past the
  end is 416 with `bytes */len`; several spans, another unit and an `If-Range` that does not hold
  (a stale tag, a weak tag, any date: the edge keeps no modification times) are answered with the
  whole file and the reason in the `Outcome`, as RFC 9110 allows. A malformed range is a 400,
  not ignored. `HEAD` ignores `Range` and reports the length `GET` would send. A pre-compressed
  variant is never ranged.
- **What the manifest does not carry.** `uses.manifest/v3` has no redirect entries, no variants
  and no lengths. Lengths come from `Blobs`. Moved routes and variants are two fields of `Deploy`
  (`Moved`, `Variants`) that an adapter fills; the folder adapter leaves both empty, since the
  written manifest's record holds the build, not the change set that knows what moved. Serving
  a 301 from a built folder needs the build to write them: a v4 manifest prefix with a
  `Status::Moved` entry whose `location` is a route, and a variant entry that names the blob it
  compresses and its coding. Until then the 301 and the `br`/`gzip` paths are tested with
  in-memory ports, and `ink serve` answers 404 for a renamed route. Not added: JavaScript.
- **Budgets are `Limits`** (0044, operator bucket 2), a struct with a `Default`: target 4 KiB, 32
  segments, 64 headers, 8 KiB a value, 32 KiB of head, 16 tags, 1 GiB a file; 10 s to send the
  head, 5 s to look a site and a blob up, 30 s idle, 10 min a response, 30 s to drain, 1024
  requests in flight and 1024 connections (0058 changed the last two and dropped the chunk). A size over its limit is a refusal with `{value, limit}`. The
  edge keeps the sizes; the adapter keeps the times, because only it has timers.
- **Why hyper under axum.** `axum::serve` exposes no header-read timeout and no header buffer,
  and a slow client is the first attack on a self-hosted edge. The accept loop is hyper's
  `http1::Builder` (timer, `header_read_timeout`, `max_buf_size`, `max_headers`) driving the axum
  router through `hyper-util`, with `GracefulShutdown`. hyper reads at least 8 KiB of head: a
  smaller `Limits::head` is a typed error at start, not a panic. hyper's own 4xx (a head over its
  buffer, bad framing) is sent without our headers; it carries no site content.
- **`/healthz`** answers `200 ok` only on a host no site has, so a probe by address works and no
  author loses a page called `healthz` (a legal slug). A probe that sends no `Host` is a 400.
- **Events, not log text.** The adapter calls an `Observe` with typed `Event`s (listening,
  answered with its `Kind`, refusal and fault as data, cut, connection error, stopping, stopped).
  `Tracing` writes them to `tracing` (refusals at warn, failures at error); `ink serve` writes
  them to stderr after a timestamp. An event holds the target without its query, cut at 200
  bytes, and no header: nothing in it is a credential or a reader's words. Peer addresses are not
  logged.
- **The folder adapter reads what `ink build` wrote.** The manifest model serializes and does not
  deserialize, so `edge/folder` mirrors the file's shape for the fields it needs, refuses a schema
  other than `uses.manifest/v3`, recomputes `Manifest::id` and refuses a deploy id that does not
  match, and checks every entry's blob is in the `files` list. Blobs are reached only through that
  list, never from a request path, so `.ink-manifest.json` and its record cannot be served and no
  request names a file the build did not write; a link on the way is refused. The manifest is read
  again when its modification time or length changes. `--host-mode` is `auto` (one deploy when
  the folder has a manifest, else a folder per host), `single` or `folders`; a host picks
  `<dir>/<host>` only as a validated DNS name.
- **`ink serve` and `ink preview`.** `preview` builds in memory on every start and serves for
  writing with a hand-written responder; `serve` serves a deploy that exists, through the code
  the product runs. It listens on 127.0.0.1:8080 unless told, stops on Ctrl-C, and is plain HTTP.

**Not done:** TLS and on-demand ACME, the API and intake surfaces, the polling watch, the worker
adapter, operator flags for `Limits`, NDJSON events under `--json`, a conformance crate shared by
adapters (the axum tests reuse the edge's stand-ins by path), per-host limits.
**Why:** the edge is the product's one internet-facing part and the thing most worth testing
without a network, so the answer is a pure function of typed input and the rest is adapters.
**Reversal:** if the Send bound on the ports blocks the worker, split the traits into a `Send`
and a local flavour behind one macro-free blanket impl; the core's logic does not change.
