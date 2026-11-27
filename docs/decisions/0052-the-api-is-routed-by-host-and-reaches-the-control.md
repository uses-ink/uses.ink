## 0052 The API is routed by host and reaches the control plane over a channel; intake takes a built folder; the watch keeps its state in memory

Built (2026-11-27) as the second half of M3's edge: `edge/api`, the polling `watch`, `clock`, the
`edge/axum` mount and `ink serve --data`. The owner has not read it yet; these are the choices to
push back on.

- **The API is a crate over the control plane, not part of the pure edge.** `edge/api` has
  `Api<P, C, K, D>` over a `Control<P>` and three ports: `Credentials` (a bearer token to an
  account), `Clock` and `Draw` (the new `clock` crate: time, a wait, 16 random bytes). Its input
  is a `Request` (method, path, `Authorization` bytes, a body that is whole or `Over { limit }`)
  and its output a `Reply`; it names no HTTP library. Routes are one enum, `Call`, parsed from a
  method and a path with every id checked, under `/v1/sites`: list, add, status, remove, deploys,
  rollback, resume, builds (list, request), a build's log as NDJSON, export as tar, and intake as
  `PUT /v1/sites/{site}/deploy`. Every failure is a `Fault` with its sources, answered as a
  `Problem` (`problem`, `message`, `causes`; a 5xx gives no causes). Authentication runs before
  routing, so a caller with no token learns no route. A site of another account is no site (404).
- **Host, not path.** The API answers on its own host (`--api-host`, default `api.localhost`),
  routed in `edge/axum` before `Edge::respond`. A path prefix on site hosts would shadow an
  author's route, the reasoning of `/healthz`; `api` is already a reserved handle. The edge also
  refuses a body on a read, which the API needs.
- **The `Send` bridge.** The control plane's ports are not `Send` (D1, R2), and a connection is a
  spawned task. `Bridge::new(api, limits)` returns a `Send` half the server calls and a `Local`
  half that owns the API and answers one call at a time on the task the caller runs it on
  (`ink serve` joins it with the server on its current-thread runtime). One call at a time is
  also what one SQLite connection does. A call is bounded in time by the body read (10 minutes).
- **Intake takes what `ink build` wrote**, as a tar archive: `.ink-manifest.json` and the files.
  The tar reader accepts regular files and folders only, refuses links, devices, pax headers,
  names that are no `ContentPath`, duplicates and bad checksums, and counts sizes as it reads.
  The manifest is checked by the folder adapter's own reader (now public in `edge/folder`):
  schema, deploy id, routes. Every listed file must be in the archive and hash to what the manifest
  says, and every entry's blob must be in the list, before anything is stored; then the blobs are
  stored under the site's tenant and `Control::deploy` records the deploy with no build. Source
  upload plus a build is the roadmap's `ink push` and `ink:source-upload`, not this. A new site
  source, `upload` (`build::Source::Upload`, stored with an empty locator), says a site takes
  intake and has nothing to fetch or build. A git source over `http` or `file` is refused by the
  API (`https` only).
- **Limits are the operator's ceilings (0044).** `edge_api::Limits` (JSON body 64 KiB, archive 256
  MiB, 10 000 files, 64 MiB a file, 1 KiB a name, export 256 MiB, 50 rows) is set by flags of
  `ink serve`; `Limits::lower(&Asked)` is what a site's configuration passes through and returns
  `Raised { limit, ceiling, asked }` for a value above the ceiling, never a silent clamp.
- **The watch.** `Watch` polls the active git sites (`Control::watched`) through a `Heads` port
  (the commit a branch is at, from the remote's reference list). A commit not seen before queues
  a build with trigger `poll` and the dedupe key `poll-<site>-<commit>`. The interval is
  `Interval { base 10 min, max 1 h }` with a floor of 10 s; a source that did not move or did not
  answer is asked half as often after each look, down to `max`; a move resets it. The loop rests
  until the first site is due, never less than 1 s. **State is in memory**: after a restart the
  first poll of each site queues a build, which is the same deploy when nothing changed, and a
  queued build of the same commit is dropped by its key. A stored "last seen" would be a new
  migration; it is not needed yet. A `Worker` takes requests off the queue and runs them, resting
  when it is empty or failing.
- **The owner.** `Owner` is the one credential adapter: it keeps the BLAKE3 digest of one token,
  compares digests in constant time, and never prints the token (`Debug` is `Token(..)`). `ink
  serve --data <folder>` makes the owner account (a fixed id) and the token in
  `<folder>/owner-token` (mode 0600), shows it once when made, and prints every setting it runs
  with. Without `--data` nothing of this runs and `ink serve` is unchanged.

**Not done** (the first two are 0067): a `Heads` adapter for a real remote, so the watch and the
worker are not run by `ink serve`; serving the control plane's own deploys; sign-in (0066); `ink push`; streaming a large export or upload instead of holding it in memory.
**Why:** the API is the product's one write surface, so it is typed values over ports with the
hostile input (a tar, a token, a path) refused by types that carry their data. **Reversal:** if one
call at a time is too slow, run several `Local` halves on a pool of connections; the bridge's
interface does not change.
