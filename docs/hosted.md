# M4: Hosted on Cloudflare, implementation plan

Status: **proposed** (decision 0079). Nothing in this document is built. It is the plan for
roadmap milestone M4, written from the repository as it is on `main`, so that agents can take
work packages without asking what exists. Section 12 lists the questions only the owner can
answer, each with a recommended default; no account, domain, bucket or bill is touched until
the owner answers them.

Paths. The repository is organised under `crates/<layer>/` (0074, `docs/layout.md`). New crates
here are named for that layout. Crate names did not change in the move, and the `Names` rule reads
the path below the layer, so `crates/port/db/d1` is `uses-ink-db-d1`.

## 1. What M4 has to produce

The roadmap's M4 exit check, split into three tests that can fail:

1. A GitHub repo goes from sign-in to live at `<handle>.uses.ink` in about a minute.
2. A push is live within a measured time (`deploy.published` already says `visible-within: 10 s`,
   `docs/architecture.md` section 11).
3. A custom domain goes live with a valid certificate.

Hosted is the same product on other adapters. `docs/architecture.md`
section 3 draws it: a Worker that serves, runs the control plane and takes webhooks; D1 for rows;
R2 for blobs; Queues for build requests; a Durable Object per handle; a Container that runs
`ink build`. Decision 0004 chose it; 0030 shaped the `Db` port like D1 so the SQLite code cannot
do what D1 cannot.

## 2. What exists, and what the port shapes already decide

All facts below were read on `main` (commit `dad76cc2`).

| Fact | Where |
|---|---|
| Every port future is allowed to be `!Send`, with a comment naming D1, R2, Queues, Durable Objects or a Workers secret binding as the reason. | `crates/port/db/src/port.rs`, `crates/port/blob/src/port.rs`, `crates/port/queue/src/port.rs`, `crates/port/build/src/port.rs`, `crates/port/secret/src/port.rs`, `crates/port/mail/src/mailer.rs` |
| `Db` is `run` and an atomic `batch` of typed statements; no interactive transaction; foreign keys enforced; a bound-parameter limit; a migration ledger. The conformance cases pin D1 behaviour. | `crates/port/db/src/port.rs`, `crates/port/db/conformance/src/lib.rs`, 0030, 0049 |
| `blob::Blob` is `put` (hashed as taken, `Mismatch` stores nothing), `get` (chunked), `stat`, `list`, `delete`, per `Tenant`. | `crates/port/blob/src/port.rs`, 0009 |
| The queue adapter `crates/port/queue/rows` is written over any `db::Db` and "runs on D1" by design: one claiming `UPDATE ... RETURNING`, leases, a dedupe key. | `crates/port/queue/rows/src/lib.rs` |
| `Build::run` takes a `Request` (build id, tenant, trigger, source, base) and an `Events` sink and returns `Built` or `Refused`; the same source gives the same deploy id. `crates/runtime/build/process` is the whole pipeline over any `Blob`. | `crates/port/build/src/port.rs`, `crates/runtime/build/process/src/lib.rs` |
| `Secret` is `seal` and `open` bound to a row id (XChaCha20-Poly1305, 0031); the key lives in the adapter. | `crates/port/secret/src/port.rs`, `crates/port/secret/local/src/lib.rs` |
| `control::Control<P>` takes four ports, `(db, blob, queue, secret)`; the build port is passed only to the use case that runs builds. | `crates/server/control/src/ports.rs` |
| The edge's `Sites`, `Blobs` and `Blob` traits are `Send + Sync`, and `Chunks` is `Send`; 0051 says `crates/server/edge/worker` "wraps these traits in its own twin" and gives the reversal (a `Send` and a local flavour behind one blanket impl). | `crates/server/edge/src/serve/port.rs`, 0051 |
| `crates/server/edge/api` is `Api<P, C, K, D>` over a `Control<P>`, takes a `Request` and returns a `Reply`, names no HTTP library, and reaches the control plane through a `Bridge` because a connection is a spawned task. A Worker needs no bridge. | `crates/server/edge/api/src/api.rs`, 0052 |
| The signed-request nonce store is a concrete in-memory `Nonces` struct with a boot barrier, not a trait. | `crates/server/edge/api/src/signature.rs`, `docs/auth.md` section 5 ("Hosted") |
| `Handle::RESERVED` holds 24 infrastructure names; the brand list is the owner's and is not there. A `tombstone` table has `handle`, `site_id`, `removed_at`; `Receipt` carries counts, never content. | `crates/server/control/src/handle.rs`, `crates/server/control/src/migration.rs`, `crates/server/control/src/receipt.rs`, 0022 |
| `watch`'s `Heads` is a trait whose comment says a Worker's fetch is not `Send`; the only adapter, `crates/server/watch/git`, uses `gix` over rustls (0072), which is not a Worker dependency. | `crates/server/watch/src/head.rs`, `crates/server/watch/git` |
| The dashboard `web` is typed view models and a `Screens` trait the edge implements; mounting it, previews, custom domains and linked identities are "not done". | 0053 |
| Container egress: the build Durable Object starts the Container with `enableInternet: false` and registers outbound intercepts from Rust through `js_sys::Reflect`; the Worker's own `fetch` is the egress policy and attaches the forge credential. Verified in workerd for DO streaming, R2 and the loopback handler; **the container itself never started under `wrangler dev` on this host** (workers-sdk #15394). | 0037, `spikes/workers/README.md` |
| `webauthn-rs-core` 0.6.2 builds for `wasm32-unknown-unknown`; it has not been run in `workerd` and its size is not measured. | 0068 build note, `spikes/passkeys/README.md` |
| `spikes/` is excluded from the workspace (own `Cargo.lock`), and the `farm` jobs are `cargo xtask check <stage>`. | `Cargo.toml`, `.config/farm.toml` |
| The folder `docs/decisions/` has no file numbered 0050 (it jumps from 0049 to 0051). Hosting is 0004, 0019, 0021, 0022, 0030, 0037; the edge, API and dashboard are 0051 to 0053. | `docs/decisions/` |

Three consequences shape the plan.

**The queue needs no new mechanism.** Cloudflare Queues delivers to a consumer Worker as a batch (push), with `retry()` and a retry
delay, and has no dedupe key. A pull consumer has a visibility timeout, but it is the HTTP API, not
something a Worker calls. The port needs `send` that answers `Duplicate` for a repeated key, and a
`receive` that takes a lease from inside the Worker (read against the Queues docs of 2026-04-21).
`crates/port/queue/rows` already implements both in rows of any `Db`, and runs on D1.
So hosted uses it as it is, over `crates/port/db/d1`, and `crates/port/queue/cf` is only a wake-up: after a `send` it
posts one message to a Cloudflare Queue so that a consumer Worker runs `receive` at once instead
of at the next Cron tick. The rows stay the truth. This is decision 0079's first choice.

**Blobs must reach R2 from a container that has no R2 binding.** A Container sees HTTP, not
bindings. Following 0037, the container's builder writes each blob with an HTTP `PUT` to a name
the egress handler owns (`blobs.internal`), and the handler, which is the Worker and holds the R2
binding, verifies the hash and puts the object under the tenant in the handler's props. The
container never holds a credential or a bucket name. The builder in the container is
`crates/runtime/build/process`'s pipeline over a `Blob` adapter that speaks that `PUT`.

**The edge twin comes first.** The Worker cannot implement the `Send + Sync` edge traits. The
split of 0051's reversal is the first edit of `crates/server/edge/worker`, in `edge`, and changes no logic.

## 3. Test strategy: what runs without an account

Cloudflare's local runtime, `workerd` through `wrangler dev` or Miniflare, simulates D1, R2, Queues,
Durable Objects, Cron Triggers and Cache locally, persisted under `.wrangler/state`. It needs
node and `wrangler` on the host, no Cloudflare account and no network after install. Containers
are the exception: they do not start under local `wrangler dev` on native Linux Docker (#15394).

| Layer | Without an account | Needs the account |
|---|---|---|
| Pure logic (handles, tombstones, budgets, abuse state, orgs, the Durable Object state machines) | Native tests on `crates/port/db/sqlite`, as `control` is tested now | nothing |
| `crates/port/db/d1`, `crates/port/blob/r2`, `crates/port/queue/cf`, `crates/port/secret/worker` against their conformance suites | Yes, in `workerd` | A real D1 and R2 run once for limits and consistency (section 3.2) |
| `crates/server/edge/worker` serving, API, dashboard, signed requests, passkey verification | Yes, in `workerd`, with `Miniflare` bindings | WebAuthn in a real browser against `app.uses.ink` (the origin must be real) |
| Durable Objects (build coordinator, nonce store) | Yes, including alarms | Eviction and cold-start timing |
| `crates/port/build/container` | The Durable Object half and the egress handler, with an `emit`-style stand-in server; the `Build` conformance suite against a harness | The container start, start time, container-to-DO stream, enforced egress; or a host where local containers start (a VM, per the issue) |
| Cron and Queue wake-ups | Yes (`wrangler dev --test-scheduled`) | Production timing |
| Deploy pipeline | `wrangler deploy --dry-run`, `wrangler d1 migrations apply --local`, bundle size | An actual deploy |
| Custom domains | The control-plane rows, the TXT proof (a fake resolver) and the hostname lookup | Cloudflare for SaaS: custom hostname creation, certificate issue, apex behaviour |
| `ink:authority-github` | Webhook signature check and token exchange against recorded fixtures | A registered GitHub App |
| Mail | `Capture` and the template tests | A sending domain, SPF, DKIM, a provider |

### 3.1 The harness gap

The conformance suites are not runnable in `workerd` as written. `suite!` turns each case into a
`#[test]` that blocks on `pollster` (`crates/port/db/conformance/src/lib.rs`), and a blocking executor cannot
drive a binding, whose futures need the JavaScript event loop. The first package, H1, adds one
thing to each conformance crate: the cases as a list of async functions that any executor can run.
A test Worker mounts that list at `/__conformance/<case>`, and a driver in `xtask` starts
`wrangler dev --local`, calls each case and reports through the existing `Report`. The native
`suite!` stays and is unchanged.

### 3.2 What a simulator cannot show

Miniflare is not D1 or R2. Items that must be rechecked once against the real services, in the
account, before the exit check: D1's integer handling through the JavaScript boundary (an `i64`
above 2^53 must round-trip or be refused as a typed error, not truncated), the bound-parameter
limit, row and statement size limits, R2's behaviour on a streamed put with an unknown length, a
large multipart upload, list pagination, and the per-request CPU and memory limits of the Worker
(the isolate is 128 MB, 0004). They are checked by a small `ink`-owned script run by hand, not a
test in the gate.

### 3.3 The gate

A sixth stage, `cargo xtask check worker`, builds the Worker crates for `wasm32-unknown-unknown`,
runs `wrangler deploy --dry-run` and drives the conformance Worker. It is a new job in
`.config/farm.toml` placed on a farm host that has node and `wrangler` (a host setting, not an
agent's). Until then `farm check` stays the five stages. The Worker crates are not default
members of the native workspace: `worker` types are `wasm32` only in part, and the native stages
(lint, test) must not compile them. Section 11 lists this as a risk.

## 4. Crates, by their post-move paths

| Crate | Path | Implements or is | Needs |
|---|---|---|---|
| `uses-ink-db-d1` | `crates/port/db/d1` (new) | `db::Db` on a D1 binding, with the `Ledger` | workers-rs `D1Database` |
| `uses-ink-blob-r2` | `crates/port/blob/r2` (new) | `blob::Blob` on an R2 bucket | `Bucket` binding |
| `uses-ink-queue-cf` | `crates/port/queue/cf` (new) | `queue::Queue`: `crates/port/queue/rows` over `crates/port/db/d1`, plus a Queues wake-up | `Queue` binding |
| `uses-ink-secret-worker` | `crates/port/secret/worker` (new) | `secret::Secret` with the key from a Workers secret | secret binding |
| `uses-ink-mail-cf` | `crates/port/mail/cf` (new, name set by owner question HQ9) | `mail::Mailer` over the provider's HTTP API | provider |
| `uses-ink-build-container` | `crates/port/build/container` (new) | `build::Build` as the per-handle Durable Object client, the egress handler's blob gateway, and `image/` (the builder entry in the container) | Containers |
| `uses-ink-edge-object` | `crates/server/edge/object` (new) | pure state machines of the Durable Objects: build coordinator, nonce store | `core`, `port` only; tested natively |
| `uses-ink-edge-worker` | `crates/server/edge/worker` (new, `cdylib`) | the Worker: `fetch`, `queue`, `scheduled`, the Durable Object classes, the twin of the edge traits | everything above |
| `uses-ink-watch-worker` | `crates/server/watch/worker` (new) | `watch::Heads` over the Worker's `fetch` (smart HTTP `info/refs`) | `worker` |
| `uses-ink-authority` family | `crates/server/authority/{github,email,dns}` (new family, name is a layout question) | the authorities of `docs/architecture.md` section 12 | `control`, ports |

The layer order holds: adapters sit in `port`, the Worker and the Durable Object logic sit in
`server`, none depends on `surface`. `MAY_USE` rows and `ENFORCED` entries for each are added in
`.config/layout.toml` (after P2) by the package that adds the crate.

## 5. Work packages

Size is agent-days of focused work: **S** about one, **M** two to three, **L** four to seven, **XL**
more, which means split it. Risk is the chance the package takes more than its size or changes the
plan: low, medium, high. "Account" means the Cloudflare account (or the named third party) is
needed to finish, not to start.

Whether a package can run in a parallel agent is in section 6.

### 5.0 Before any code

| Id | Package | Owns | Notes |
|---|---|---|---|
| H0 | Owner decisions and the Cloudflare setup (section 12) | no repo path; `docs/hosted.md` section 12 | Size S, risk low. The owner answers; an agent can write down the answers. Account steps are owner-run (secrets through `secrets_request`, never pasted). |
| H0b | Spikes that need nothing from the workspace | `spikes/hosted/**` (new, excluded from the workspace like `spikes/workers`) | Size M, risk medium. See below. |

H0b answers four unknowns while the layout work runs, in a folder the layout moves do not touch:

1. **Size and start of the Worker.** Build a Worker with `crates/server/edge/api`'s dependencies including
   `webauthn-rs-core` for `wasm32-unknown-unknown` and `worker-build --release`; record the wasm
   size (raw and gzip) against the plan's bundle limit, and run a passkey ceremony verifier in
   `workerd`. Closes the open "run in workerd" of 0068.
2. **D1 value fidelity.** `i64`, `u64` as text, blobs, `NULL`, and the bound-parameter limit through the
   JavaScript boundary of workers-rs 0.9.
3. **R2 streaming put with hashing**, unknown length, over the limits of a Worker request body.
4. **A real container start** on a host where it works (a Debian VM per #15394) or on the
   account, giving start time and the container-to-DO stream, which 0037 left unverified. Needs the
   account or a VM, so it can wait for H0.

Each spike ends with a yes or no and numbers, as M0's did, into a decision that supersedes or
amends 0079.

### 5.1 The foundation (serial)

| Id | Package | Owns | Implements | Conformance | Without account / account | Size, risk |
|---|---|---|---|---|---|---|
| H1 | Test rig and `crates/port/db/d1` | `crates/port/db/conformance` (async case list), `crates/port/db/d1`, `tools/xtask` (stage `worker`), `.config/farm.toml` (`[job.worker]`), the test Worker `crates/server/edge/worker/tests` skeleton | `db::Db`, `db::Ledger` | `db-conformance` (all cases, pinned to D1 by 0030) | all in workerd; account only for 3.2 | M, medium. The D1 value boundary is the risk. |
| H1b | Worker migrations in wrangler's format | `crates/port/db/src/render.rs` use from xtask, `migrations/` (generated) | the renderer already used for SQLite | the rendered file applies with `wrangler d1 migrations apply --local` and gives the schema `crates/port/db/sqlite` gives | workerd | S, low. 0030 says "one migration set in wrangler's format"; 0049 forbids raw SQL text in code, so the files are generated. |

H1 is serial and first: it is the rig every later adapter is tested on. H1b follows it.

### 5.2 Adapters (parallel after H1)

These own disjoint crates and share only the root `Cargo.toml`, `Cargo.lock` (regenerated, never
merged by hand), `.config/layout.toml` rows and the `worker` stage's driver list.

| Id | Package | Owns | Implements | Conformance | Test locally | Account | Size, risk |
|---|---|---|---|---|---|---|---|
| A1 | `crates/port/blob/r2` | `crates/port/blob/r2`, `crates/port/blob/conformance` (async list) | `blob::Blob` | `blob-conformance` (round trips, sizes, mismatch stores nothing, listing, delete, per-tenant isolation) | yes, Miniflare R2 | Real bucket for streamed put and multipart (3.2) | M, medium. A put hashes as it takes chunks and must not hold a large blob whole; R2 needs a known length or a multipart upload. |
| A2 | `crates/port/queue/cf` | `crates/port/queue/cf`, `crates/port/queue/conformance` (async list) | `queue::Queue` | `queue-conformance` (the rows' cases, plus: a wake-up is sent after `send` and never changes the answer) | yes | Real Queue for delivery timing | S, low. If the owner takes the default (Cron wake-up only), A2 shrinks to a configuration entry. |
| A3 | `crates/port/secret/worker` | `crates/port/secret/worker`, `crates/port/secret/conformance` | `secret::Secret` | `secret-conformance` | yes (a local secret in `.dev.vars`) | A real secret binding; key rotation procedure written down | S, low. Shares the sealing code of `crates/port/secret/local`; the key source is the only difference, so first lift the seal into the port crate or a shared crate (a layout question, one line). |
| A4 | `crates/port/mail/cf` | `crates/port/mail/cf` | `mail::Mailer` | none exists for `mail`; add a small suite (message with a link, bad address refused, failure typed) that `Capture` and `Stdout` also pass | `Capture` | Sending domain, SPF, DKIM, DMARC, provider account | S, low. Blocked on owner question HQ9 only for the provider's API. |
| A5 | Nonce store, extracted and in a Durable Object | trait `Nonces` in `crates/server/edge/api` (the extraction), `crates/server/edge/object/nonce`, the DO class in `crates/server/edge/worker` | `Nonces` (new trait, async, `claim` bounded per key, boot rule) | a new `nonces` suite in `crates/server/edge/api/tests` that the memory store and the DO both pass: first claim wins, replay refused, entry gone after `c + skew`, bounded per key, boot barrier | pure state machine natively; the DO class in workerd | Eviction timing | M, medium. The `crates/server/edge/api` split (S1) and passkeys have landed; per-account DO versus a D1 unique key is decision 0079's third choice. |
| A6 | `crates/server/watch/worker` | `crates/server/watch/worker` | `watch::Heads` | a fixture of `info/refs` bodies of GitHub, Forgejo, GitLab (recorded) read the same as `crates/server/watch/git` reads a local repository | yes, with a fake `fetch` | Real forges | S, low |

### 5.3 The Worker, the builder, the pipeline

| Id | Package | Owns | Depends on | Conformance | Account | Size, risk |
|---|---|---|---|---|---|---|
| W1 | Edge twin and `crates/server/edge/worker` serving | `crates/server/edge/src/serve/port.rs` (the `Send` and local flavours, no logic change), `crates/server/edge/worker` (serve path: host to site, manifest blob, entry blob, streaming, range) | H1, A1 (can start on in-memory fakes); edge twin first | a new `edge-conformance` crate for `Sites` and `Blobs` adapters (0051 lists the absence); the existing edge tests run through the Worker via `Miniflare` | Cache API timing only | L, medium. The host lookup is cached per colo for at most 10 s, and the manifest is fetched per deploy, not per request: both through the Cache API keyed by deploy id. |
| W2 | API, dashboard, sign-in on the Worker | `crates/server/edge/worker` (`api`, `dashboard`, `signin` modules), `crates/surface/web` (the `Screens` implementation if it lives there) | W1, A3, A4, A5, M3's dashboard mount (0053 "not done") | the API tests of `crates/server/edge/api` and the dashboard goldens, run on workerd | Passkeys in a real browser on a real origin | L, high. Passkeys unrun in workerd until H0b, size of the bundle. |
| B1 | The per-handle build Durable Object and the coordinator logic | `crates/server/edge/object/build` (state machine: one build at a time, lease, attempt count, the live log), the class in `crates/server/edge/worker` | H1, A2 | pure state machine under native property tests (never two builds at once, a crash leaves a diagnosable row, the log is the typed `build::Event` stream, 0023) | Real alarms timing | M, medium |
| B2 | `crates/port/build/container` and the container image | `crates/port/build/container` (DO-side `Build`, the egress handler's blob gateway and credential attachment), `crates/port/build/container/image` (the builder entry and `Dockerfile`), `crates/port/build/conformance` (async list) | B1, A1; the builder pipeline of `crates/runtime/build/process` with an HTTP `Blob` adapter | `build-conformance` (builds and stores what it names; same site same deploy; refuses a bad site and says why; leaves nothing under another tenant) against a harness; plus a test that the gateway refuses a put whose hash differs and a tenant that is not in the props | **Yes**, to start the container, measure cold start (0004's reversal threshold is about 15 s push-to-live) and enforce egress | XL, high. Split: B2a the gateway and handler (workerd), B2b the image, B2c the real start. #15394 blocks local runs here. |
| B3 | Hosted watch, webhooks, Cron | `crates/server/edge/worker` (`scheduled`, webhook routes), `crates/server/authority/github` (signature check, installation token per build, never stored, commit status) | B1, A6, W2 | recorded GitHub webhook fixtures; the `Heads` suite | A registered GitHub App | L, medium |

### 5.4 Control-plane features (native-testable, parallel)

All of these are `control` work tested on `crates/port/db/sqlite`, and so need no Cloudflare. Each adds its
own migration in its own module, following `queue::Queued` and `control`'s `Domain`, with the order
of migration ids assigned up front so the packages do not collide. The split into `crates/server/control/rows`, `crates/server/control/signin`
and `crates/server/control/keys` (S2) has landed, so they can start as soon as the paths are final (S6) or, if the
owner allows, on today's paths (section 6).

| Id | Package | Owns | Ports | Conformance and tests | Size, risk |
|---|---|---|---|---|---|
| C1 | Handles: brand reservations, holds, tombstones | `crates/server/control/rows` (the `Reserved` migration, `Handle::reserve`, the hold), data file `reserved-handles.toml` read once | `Db` | properties: a tombstoned handle is not claimable for its hold; a verified handle is never reclaimed; first claim wins across forges; a reserved name is refused with the typed reason; the same on D1 through H1's rig | M, low. The brand list and the hold are owner answers (Q5, O3). |
| C2 | Budgets and the limits page | `control` (`Budgeted` migration, counters: bytes stored, builds a day, sites), `crates/surface/web` (the limits page rendered from the same typed `Limits` the refusals use, 0021, 0044) | `Db`, `Blob` (`list` for sizes) | a refusal carries `{value, limit}`; the page and the refusals read one value; a deploy above budget is refused whole (Q6, all or nothing) | M, low. Numbers are an owner answer (O7, Q13). |
| C3 | Deletion with receipt, hosted | `control` (resume and bounds), `crates/server/edge/worker` (a Durable Object alarm drives a long delete), the receipt mail | `Blob`, `Db`, `Mailer` | the removal tests of `control` with a blob adapter that fails halfway: a crash leaves a diagnosable row, never orphaned bytes (0022); `Blob::list` after removal is empty; `Receipt` carries counts only | M, medium. R2 deletes in batches and a Worker has a time limit, so a delete is a resumable job, not one request. |
| C4 | Abuse report and takedown | `control` (`Abused` migration, `Suspension` state of a site), `crates/server/edge/api` (`POST /v1/abuse`, unauthenticated, rate limited), `edge` (a suspended site answers a typed status and a reason code, no content) | `Db`, `Mailer` | a suspended site serves the notice and no bytes; a report is rate limited per source; the operator action needs the operator key (0068); every action is an `auth_event` row | M, medium. Legal text and the contact are owner answers (HQ10). |
| C5 | Orgs and roles | `crates/server/control/keys` and `crates/server/control/rows` (`Orged` migration; `Org`, `Member`; roles are code, `docs/auth.md` section 12) | `Db` | the role table of `auth.md` as a property: a Writer cannot `Members`; the last owner cannot be removed; an org's deploy key outlives its creator | M, medium. Waits for S2 and the `passkeys` work. |
| C6 | Previews of branches | `control` (`Previewed`: a deploy kind with a label and an expiry), `watch` (branches), `edge` (host `<label>--<handle>.uses.ink`), `web` (the screen) | `Db`, `Blob`, `Queue` | a preview expires and its blobs are swept; it is unlisted; it cannot name a real handle | M, medium. Handles have no `--` (architecture section 12), so `<label>--<handle>` cannot collide with a handle. Q7 decides who may see one. |
| C7 | Custom domains rows and the TXT proof | `control` (`Domained` migration: the `domain` table of architecture section 11), `crates/server/authority/dns` (the TXT `_uses-ink.<name>` proof bound to a handle), `crates/server/edge/control` (the host lookup includes custom hostnames) | `Db`, a `Resolver` port (new, small, with a fake) | proof binds name to handle; a second handle cannot take a proven name; a removed domain stops resolving; a tombstoned handle's domain is released | M, medium |
| C8 | `ink:authority-email` | `crates/server/authority/email` | `Mailer` | the magic link of 0066 (single use, 15 minutes), as authority proof of an address | S, low |

Previews and the decisions diff need `web` screens that do not exist (0053). C6 adds the preview
screen only; the diff is not M4.

### 5.5 Needs the account (serial, mostly)

| Id | Package | Owns | Needs | Size, risk |
|---|---|---|---|---|
| L1 | Deploy pipeline | `wrangler.toml` (committed, one per environment), `.github/workflows/deploy.yml`, `tools/xtask` (a `deploy` verb that prints what it will do and refuses without `--yes`), `docs/hosted.md` runbook | Cloudflare account, API token scoped to the account, GitHub environment secrets. Containers need Docker in the runner. `wrangler d1 migrations apply`, `wrangler deploy`, container image push. A deploy is gated by `farm check` and the `worker` stage. | M, medium |
| L2 | Custom domains live | `crates/server/edge/worker` (`/v1/domains`), `crates/server/authority/dns`, a Cloudflare API client behind a port `Hostnames` (new, with a fake for the suite) | Cloudflare for SaaS on the zone, the fallback origin `domains.uses.ink`, an API token that can only edit custom hostnames. Certificate validation and the `www` plus redirect rule for apex (0019, O5). | L, high. Apex without CNAME flattening is a stated limit. |
| L3 | Hosted mail live | `crates/port/mail/cf` configuration | the provider, the sending domain's DNS | S, low |
| L4 | The exit check | `crates/surface/cli/tests` or a script under `tools/xtask` that signs in, adds a repo, times it to live, pushes, times it, adds a domain | everything above in a staging environment | M, medium. The three tests of section 1 are written first as failing checks so that L1 to L3 have a target. |
| L5 | Observability and runbooks | `crates/server/edge/worker` (`Observe` adapter writes typed events to Workers Logs; no request bodies, no peer addresses, 0051), `docs/hosted.md` runbooks: rotate a secret, restore L1 from Time Travel, stop a runaway build, suspend a site | Cloudflare | S, low |

## 6. Order, parallelism and the layout freeze

```
now (no new workspace crate):   H0   H0b (spikes/hosted)        layout: S3, S4, S5 (P1 to P6, S1, S2, D1 to D3 and passkeys have landed)
S6 freeze, one atomic commit:   nothing in M4 adds a crate or edits a moved path across it
after S6:                       H1 -> H1b
                                A1  A2  A3  A4  A6        (parallel, after H1)
                                A5 (needs the crates/server/edge/api auth/ split: landed)   W1 (needs the edge twin, H1)
                                B1 -> B2a -> B2b -> B2c
                                C1..C8 (parallel; the crates/server/control/rows split has landed; sqlite-tested; need no H1)
                                W2 (needs W1, A3, A4, A5)   B3 (needs B1, A6, W2)
needs the account:              L1 -> L2, L3 -> L4 -> L5
```

Rules that follow from `docs/layout.md`:

- **Wait for S6 to add workspace crates.** S6 is one atomic commit that moves every crate and edits
  `Cargo.toml`, `ENFORCED`, `MAY_USE` and the farm `inputs`; section 13 says "S5 and S6 are never
  parallel with anything". A new crate in flight at the freeze would have to be moved by hand.
  Before S6, M4 work is limited to the owner's decisions, `docs/`, and `spikes/hosted/**` (an
  excluded folder with its own `Cargo.lock`, like `spikes/workers`).
- **What has landed, and what has not.** On `main` at `6219cd6b` the log shows P1 to P4 and P6, S1
  (`crates/server/edge/api` has `auth/`, `sites/`, `archive/`), S2 (`crates/server/control/rows`, `crates/server/control/signin` and
  `crates/server/control/keys` are workspace members), D2, D3 (the docs checks run) and passkeys (0068 slice 2).
  P5 appears landed (`crates/runtime/host/native/src/fetch/`). Not landed: S3 (one `Chain` and one `Written` in
  `model`), S4 (the `cli` split; `crates/surface/cli/src` has 43 entries), S5 and S6. Re-read the log before
  acting. `cli` is S4's until it lands, so nothing in M4 edits it. The first edit of W1 (the `Send`
  twin in `edge`) waits for S6, since it is a change to a crate S6 moves.
- **Use post-S5 names.** `queue/local` becomes `queue/rows`, `build/local` becomes `build/process`
  and `trace` becomes `logging` (S5). This plan uses the new names; before S5 read the old ones.
- **Parallel is real only where paths are disjoint.** A1, A2, A3, A4 and A6 own their crates and
  differ only in the root `Cargo.toml`, `Cargo.lock` and `.config/layout.toml`, which each
  regenerates on rebase. C1 to C8 share `control`'s error type and `Control` methods: each adds
  its own module and migration and a line to `error`; take the migration ids from a list fixed in
  H0 and rebase serially at the end. Serial: H1 (everything depends on the rig), B1 to B2c (one
  chain, one container), W1 to W2 to B3 (one Worker crate; the same files), and all of 5.5.
- **One owner for the gate.** The `worker` stage and the `[job.worker]` farm entry are one commit in H1, owned
  by H1, so no other package edits `xtask` for it.

M3 leftovers that M4 depends on and the plan does not do: the dashboard sign-in screens and the
mounting of `Screens` in the edge, streaming intake (whole-body now, within the `tree` and `part`
limits, 0077), the step-up approval and recovery of passkeys (0068), and `ink serve`'s TLS and
ACME (hosted has Cloudflare TLS, so ACME is not needed there). W2 assumes the mounting lands in
M3 (`docs/roadmap.md` lists it open). If it does not, it is the first task of W2.

## 7. Per-handle Durable Object

One class, `Build`, one instance per handle, named by the handle. It exists so that at most one
build of a handle runs at a time, the live log has one writer and the container has one owner
(0004, `docs/architecture.md` section 3).

- State: the current build id, its lease, its attempt count, the last events kept for the stream.
  Stored in the Durable Object's own storage; the truth of builds stays in D1 (`build` rows).
- It claims a request from the queue rows (`crates/port/queue/rows`, one claiming statement), starts the
  container with `enableInternet: false`, registers the outbound intercepts, streams the builder's
  events to the dashboard as the typed `build::Event` stream, writes the log to R2 at the end, and
  acks or releases. A crash leaves a lease that ends, and the request is delivered again.
- The logic (which state follows which event, what a second `start` does) is the crate
  `crates/server/edge/object/build`, written without `worker` types so it is tested natively with property
  tests. The class in `crates/server/edge/worker` is the thin shell.

The nonce store is a second class, one instance per account (`docs/auth.md` section 5): `claim`
after the signature verifies, bounded per key, evicted with the window, and the boot barrier on
every construction. A per-account object is chosen over a D1 unique key because a claim is one
round trip to one place with no table to sweep; the cost is a hop on every signed API call. Both
satisfy the same `Nonces` suite, so the choice can be reversed.

## 8. Handles, domains, budgets, deletion, abuse, previews (what is new)

**Handles (C1).** `Handle::RESERVED` has 24 infrastructure names. M4 adds the brand list (owner
Q5), a hold on a deleted or renamed handle (default 12 months), and the rule that a verified
handle is never reclaimed. The `tombstone` table has the handle, the site and the removal time;
the hold needs a `held_until` and the reason. First claim wins across forges, so the lookup is by
the handle only.

**Custom domains (C7, L2).** Our TXT proof binds a name to a handle (`authority-dns`), then the
Cloudflare for SaaS custom hostname is created and its certificate validated; the customer's
CNAME points at `domains.uses.ink` (0019). The Worker's host lookup treats a custom hostname as a
key into the same `domain` table. `Hostnames` is a port with a fake, so the control plane is tested
without Cloudflare; the real adapter is the one account-gated piece.

**Budgets (C2).** Every limit refuses with `{value, limit}` (0021) and is on a public page built
from the same values (Q13). Hosted adds counters: stored bytes, builds, build time, sites, and
the concurrency cap per handle and global. There is no hard spend cap at Cloudflare, so our caps
are the cost control (section 12, HQ3).

**Deletion (C3).** Unpublish, delete blobs under the tenant, delete rows, send the receipt (0022).
Hosted caveats stated in the receipt text: the host lookup cache means a deleted site can answer
for up to 10 s, and D1 backups age out per HQ14.

**Abuse (C4).** `POST /v1/abuse` takes a URL and a reason, is rate limited and unauthenticated (the
reader pages have no forms, `docs/architecture.md` section 13, so the endpoint lives on the API host).
A takedown is a suspension: the site answers a typed notice and serves no bytes, the content is
kept for the owner to appeal, and an operator-key action records an event. Deleting is a separate
step.

**Previews (C6).** `<label>--<handle>.uses.ink`, unlisted, an expiry (Q7 default 7 days), swept by
the same retention sweep that keeps the last 20 deploys.

## 9. Deploy pipeline

- `wrangler.toml` per environment, committed, with the Durable Object classes and their
  migrations, the D1, R2 and Queue bindings, the Container definition, the Cron Trigger and the
  routes. No account id and no secret in the file; ids come from the environment.
- CI: `farm check` and `cargo xtask check worker` pass, then `wrangler d1 migrations apply`, then
  `wrangler deploy`, from a GitHub environment with a token scoped to the one account. A migration
  that is not backwards compatible with the running Worker is a typed refusal in the migration
  review (D1 has no downtime-free schema change for a breaking one; deploy the Worker that reads
  both, migrate, deploy the Worker that reads the new one).
- Staging and production are two environments of one account with distinct resource names (HQ2).
- Rollback of the Worker is `wrangler rollback`; rollback of data is D1 Time Travel, which is also
  the answer to HQ14's backup retention (verify the retention window on the pricing page before
  the owner relies on it).

## 10. Exit check, mapped

| Exit line | Packages | How it is measured |
|---|---|---|
| Sign-in to live in about a minute | H1, A1 to A5, W1, W2, B1 to B3, L1 | L4 times the stages from the typed events (`signin`, `source added`, `build started`, `deploy published`); the cold start of the container is the biggest term (0004's threshold: about 15 s) |
| A push is live within the measured time | B1 to B3, W1 | the push-to-`deploy.published` interval, plus `visible-within: 10 s` |
| A custom domain goes live with a valid certificate | C7, L2 | L4 adds a domain of the owner on a throwaway handle and fetches over TLS |

## 11. Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Containers do not start locally (#15394) | B2 cannot be tested on this host | a Debian VM host or the account; the DO half and the gateway are tested in workerd with a stand-in server |
| Worker bundle size with `webauthn-rs-core` and the control plane | the Worker cannot deploy at the plan's limit | H0b measures first; the API and the reader path can be two Workers sharing crates if one is too big |
| D1 value boundary (`i64`) | silent truncation | typed error in `crates/port/db/d1`, the conformance case exists, H0b checks |
| `!Send` futures and the `Send` edge | the twin is the first edit of W1 | 0051's reversal, no logic change |
| Worker crates in the native workspace | the native stages run clippy, nextest and deny with `--workspace` (`tools/xtask/src/check/cargo.rs`), so a `wasm32`-only crate would break them | H1 settles it: members whose whole body is `cfg(target_arch = "wasm32")`-gated (native sees an empty crate and the layout rules still cover them), or crates excluded from the workspace like `spikes/` (no layout coverage, own lockfile). Recommended: the gated members. The `worker` stage builds them for wasm. |
| Queue semantics | wasted work on a Queues adapter that the port cannot use | rows on D1 plus a wake-up (0079) |
| Container egress intercept not accepting the loopback in hosted runs | builds cannot reach the forge with a credential | 0037's reversal: a JS shim for the handler only |
| Cold start above 15 s | exit check fails | 0004's reversal: a wasm fast path for small snapshots |
| Spend with no hard cap | surprise bill | budgets in code, build concurrency caps, Cloudflare usage notifications (HQ3) |
| Single account, single point | token leak is total | scoped tokens, 2FA, a separate deploy token, rotation runbook in L5 |

## 12. Owner decisions, costs and irreversible steps

Nothing here has been done. Questions named HQn are this plan's; Q and O numbers refer to
`docs/open.md`. Each question has a recommended default; if the owner says "defaults",
H0 records them and the packages that need an account can start. Prices and limits of Cloudflare
change: the agent re-reads the current pricing and limits pages with `curl` before the owner pays
for anything, and puts the numbers in the answer.

| # | Question | Recommended default | Cost or irreversible? |
|---|---|---|---|
| HQ1 | **The Cloudflare account.** Whose account, which email, which 2FA? | A new account under a role email the owner controls, a hardware key as 2FA, the owner as the only member, and no personal token committed anywhere. | Free to open; the account owns everything, so who owns it is hard to change later. |
| HQ2 | **Environments.** One account with `staging` and `production`, or two accounts? | One account, two environments, resource names suffixed. Staging uses `workers.dev` hostnames; no second domain. | Cheaper; a second domain would also need its own PSL entry. |
| HQ3 | **Billing.** Plan, spend alerts, container and build caps. | Workers Paid (needed for Containers), usage notifications at set thresholds, build concurrency per handle 1 and global 3 until measured, the budgets of HQ8 as the real cap. | A monthly base plus usage; **Cloudflare has no hard cap**, so our caps are the control. The owner sets the monthly ceiling that makes them alert. |
| HQ4 | **Domain and DNS.** Move the `uses.ink` zone to Cloudflare DNS? | Yes, a full zone on Cloudflare (Cloudflare for SaaS needs it), after the PSL entry (`docs/psl.md`) is accepted or confirmed pending. | **Changing nameservers is a cutover, reversible but visible**; the PSL entry takes weeks and is outward-facing, already in `docs/psl.md`. |
| HQ5 | **R2 buckets.** Names, location, jurisdiction. | One bucket per environment (`uses-ink-blobs-staging`, `uses-ink-blobs`), default location. **Ask before creating**: an EU jurisdiction is chosen at creation and cannot be changed. | Storage priced by use, no egress fee; **jurisdiction is irreversible**, and a bucket's name is hard to reuse. |
| HQ6 | **D1 database.** One or several, location hint. | One database per environment, no read replicas at launch. | Free tier then usage; one D1 has a size limit, which sets when sharding by handle range becomes a task (not M4). |
| HQ7 | **Cloudflare for SaaS.** Enable it? Apex policy (O5). | Yes, with the apex limit as in 0019: apex only where the customer's DNS flattens CNAMEs, else `www` plus a redirect, said plainly. | Priced per custom hostname after an allowance; the owner confirms the current rate. Needs a payment method on the zone. |
| HQ8 | **Budgets and the limits page (O7, Q13).** One tier? Which numbers? | One tier at launch, exact numbers on a limits page. Starting numbers, to adjust: 1 GiB stored per account, 60 builds a day, 10 minutes a build, 20 deploys kept (O6), one site per handle (HQ4). These are placeholders, not measured. | Free to decide; a free tier with real storage is a recurring cost. |
| HQ9 | **Hosted mail provider.** Which one, which sender? | One transactional provider with an HTTP API, behind `mail::Mailer`; sender `no-reply@uses.ink`; SPF, DKIM and DMARC set up with the DNS in HQ4. `mail` is already a reserved handle. | A provider account (outward-facing); **DNS records on the zone**; reputation is slow to repair if misconfigured. |
| HQ10 | **Abuse and takedown.** Contact, policy, legal agent. | `abuse@uses.ink` read by the owner, the report endpoint of C4, suspend first, delete later, written policy on the limits page. Whether to register a copyright agent is the owner's legal call. | Possible legal filing and fee; the text is public and hard to retract. |
| HQ11 | **Handles (O3, Q5).** Brand list; hold on an old handle. | A short brand list the owner writes; 12 months; never reclaim a verified handle. | A reserved name is cheap to add and awkward to take back from a person who held it. |
| HQ12 | **Previews (Q7).** Who can see a preview? | Public by URL for public sources, unlisted, 7 days; private sources never. | Free; public by URL is a disclosure choice. |
| HQ13 | **GitHub App.** Register it, with which permissions? | Yes, one App: read contents and metadata on chosen repos, write commit statuses, webhooks. Secrets (private key, webhook secret) go in through `secrets_request`, never into a file or a chat. | Outward-facing (a public App name on GitHub); the App id and name are hard to change. The owner registers it. |
| HQ14 | **Backups (Q14 of `open.md`).** Retention after a deletion. | Overwritten within 30 days, stated in the receipt; D1 Time Travel as the mechanism (verify its window). | Free; a promise to users. |
| HQ15 | **Launch gate.** Who can sign up when M4 ends? | An allowlist (invite) on `uses.ink` subdomains until M5's landing; `sign-up policy` is already a setting of `control::signin` (0066). | Free; keeps the first real traffic small. |
| HQ16 | **Telemetry.** What is collected? | Typed events to Workers Logs for seven days, no request bodies and no peer addresses (0051); no web analytics on reader pages (they run no JavaScript). | Free tier; logs are a retention promise. |

Irreversible or outward-facing steps, in the order they would happen, none taken:

1. Open the Cloudflare account and set 2FA (HQ1). Reversible only by closing the account.
2. Add the `uses.ink` zone and change nameservers (HQ4): a visible DNS cutover.
3. Create R2 buckets (HQ5), choosing the jurisdiction once.
4. Enable Workers Paid and Cloudflare for SaaS (HQ3, HQ7): recurring charges.
5. Register the GitHub App (HQ13) and the mail provider (HQ9): public names and DNS records.
6. Deploy the first Worker to production (L1): public hostnames start answering.
7. Send the first real email.

An agent does none of these. It writes the runbook, prints the exact command, and the owner runs
it or tells the agent to. Tokens come through `secrets_request`.

## 13. What is not in M4

- OIDC trusted publishing for CI (open choice 6 of `docs/auth.md`, after launch).
- A decisions diff between deploys (0053).
- `ink:authority-forgejo` and `ink:authority-gitlab` (M5).
- Sharding D1 by handle range, a second region, a paid tier (O7 default).
- The versioned inference rules of 0042 (`site.rules`, `ink update`) listed first in M4 of the
  roadmap: they are engine work, testable natively, and take no hosted dependency. They can run
  as their own package beside this plan; they are not in this document's order.
