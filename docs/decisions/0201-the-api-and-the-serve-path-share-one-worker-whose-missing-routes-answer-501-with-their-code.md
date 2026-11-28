---
number: "0201"
title: "The API and the serve path share one Worker, whose missing routes answer 501 with their code"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0201 The API and the serve path share one Worker, whose missing routes answer 501 with their code

Built (2026-11-28), the API part of W2 of `docs/hosted.md`, over W1 (decision 0200).

- **One Worker, two kinds of host.** `crates/server/edge/app` is the product Worker. A request for
  `API_HOST` is the API; any other host is a site, served as 0200 says. `ORIGIN`, `API_HOST`,
  `AUDIENCE`, `OPERATOR_KEYS` and the limits are variables (`Hosting` in `edge/worker/src/api`), each
  missing or unusable one a typed `Unready` that names it. The bindings are `DB` (D1), `BUCKET` (R2),
  `WAKE` (Queues), `NONCES` (the Durable Object) and the secret `SEALING_KEY`. `wrangler.toml` has no
  account id and no secret, and `README.md` there lists what a deploy needs.
- **The API is the same code as `ink serve`'s.** `Hosting::wire` builds `Api`, `Signatures`, `Enrol`
  and `Hooks` over the Worker's ports, so the routes, the refusals and the limits are the self-host's.
  The nonces of signatures and of webhooks are two `Remote` handles on the nonce object. The
  operator's account is made by the first request of an isolate and never again. A body is read up to
  the budget of its route, and `Content-Length` over it is refused unread.
- **The boot barrier is the store's.** `Signatures` closes the door for one window after it is built,
  which on a Worker is every cold start of an isolate. The nonce object keeps the same rule from when
  it was built (0135), and it outlives isolates, so `Signatures::booted` puts the verifier's own
  barrier at the epoch and the object's stays. Reversal: drop the call, and each cold start answers
  `auth.server.restarting` for a window.
- **Not yet is a status, not an absence.** `Status::NotImplemented` (`501`) and the catalog code
  `worker.route.pending` with its fix answer the dashboard, sign-in by link and its sessions,
  passkeys and recovery codes, and the request of a build. `Gate` puts them in front of the API, so a
  route that is planned is never a `404` and a request for a build is not queued for a builder that
  is not there.
- **Passkeys are gated off, not patched.** Registration needs the clock patch of
  `spikes/hosted/vendor` (0096), which is a copy of a published crate with one change. Bringing it
  into the tree is its own decision, so the Worker refuses the routes with the code above until then.
- **Migrations are the typed ones.** `Migrations::steps()` is the one list (the same as the
  self-host's), the `scripts` bin renders it to `NNNN_name.sql` as 0098 says, and
  `cargo xtask worker migrations` writes the files where `wrangler.toml` points `migrations_dir`;
  wrangler applies them, and the Worker never migrates.
- **One test drives all of it.** `cargo xtask worker app`, a gate case, builds the Worker, applies the
  migrations to a local D1, starts `wrangler dev --local` and runs
  `crates/surface/cli/tests/hosted.rs`: the real `ink` signs the creation of a site, `ink build`
  makes the folder, a raw signed client puts its archive through intake, the serve path answers a page,
  a range and a `404`, a replayed signature is refused, a suspended site answers `403` with none of its
  bytes, and a route that is not there answers `501`. A key asks to be enrolled and waits.

**Numbers** (farm-a, workerd through `wrangler dev --local`): the Worker is `index_bg.wasm` 4182500
bytes raw, 1207863 gzip; `wrangler deploy --dry-run` uploads 4139.73 KiB, 1204.17 KiB gzip. A request
for a route not there takes 7 ms, an enrolment 16 ms, a signed call 14 ms, a replay refused 12 ms, an
intake of a 24576 byte archive 113 ms, and a served page 20 ms cold and 12 ms warm. `LOOKUP_TTL`
(seconds, default ten) is how long the serve path remembers a live deploy, so a suspension applies
within it; the test sets it to zero.

**Not done:** the approval of an enrolment by a person, which needs sign-in (a mailed link through
`Fake` mail and the courier on `wait_until`), so only the operator's keys sign today; passkeys; the
dashboard; builds, so `ink push`, which asks the server to build, does not finish on the Worker and a
built folder is put through intake instead; a `ReadableStream` reader that stops at the budget rather
than at the first chunk past it; custom domains' DNS proof (the resolver is offline).
**Reversal:** as above, per item.
