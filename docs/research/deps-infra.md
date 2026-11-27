# uses.ink: dependencies for the control plane and the edge

Scope: crates and platform pieces for `uses-control`, `uses-edge-cf` (the Worker) and
`uses-edge-std` (`ink serve`), plus release and test tooling. The engine and builder
crates are in `deps-engine.md`. Versions, release dates, licenses and download counts were
read from the crates.io API on 2026-11-26. Platform facts were read from the Cloudflare
docs (`/index.md` pages) the same day. workers-rs claims were checked in the clone at
`~/code/strangers/workers-rs` (commit `a61be78`, 2026-11-17, version 0.9.0).

How wasm support was checked: a scratch workspace with one member per candidate, each run
through `cargo check --target wasm32-unknown-unknown` with getrandom's `wasm_js` backend
enabled. "Compiles" in the tables means that check passed; it does not mean the crate
behaves correctly inside workerd. Advisories come from `cargo deny check advisories` on
the same workspace. Anything not checked one of these ways is marked `[unverified]`.

Columns: **wasm32** is the result of the check above. **Fit** is how well it matches the
ports design (`Db`, `Blobs`, `Queue`, `Builder`, `Secrets`, `Clock`, plus an `Http` port
this document proposes). **Risk** is what could hurt us.

## 1. workers-rs 0.9

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `worker` | 0.9.0 | 2026-11-11 | Apache-2.0 | Compiles with `d1`, `http`, `axum`, `queue` | Covers every binding we need except two (below) | 205 open issues; README still says "expect rough edges"; depends on `chrono` (now flagged unmaintained, RUSTSEC-2026-0543) |
| `worker-build` | 0.9.0 | 2026-11-11 | Apache-2.0 | Tool | Downloads `wasm-bindgen` and `wasm-opt`, emits `build/index.js` | Pins a wasm-bindgen version; our `wasm-bindgen` dependency must match it |
| `worker-macros` | 0.9.0 | 2026-11-11 | Apache-2.0 | Yes | `#[event(fetch\|scheduled\|queue\|email)]`, `#[durable_object]` | None beyond `worker` |

Binding by binding (from `worker/src`):

| Need | Status in 0.9.0 | Evidence |
|---|---|---|
| D1 | `prepare`, `bind`, `first`, `all`, `raw`, `run`, `batch`, `exec`, `dump`, read-replica sessions (`with_session`, bookmarks) | `d1/mod.rs`. No interactive transactions: D1 has none (issue #349 open since 2023); `batch` is the only atomic unit ([D1 docs](https://developers.cloudflare.com/d1/worker-api/d1-database/): "Batched statements are SQL transactions") |
| R2 | get, put, list, delete, streaming body (`ObjectBody::stream`, `response_body`), multipart (`create_multipart_upload`, `resume_multipart_upload`, `upload_part`) | `r2/mod.rs`. No presigned URLs from the binding: CLI uploads need S3 SigV4 presigning with R2 access keys (see 1a) |
| Queues | Producer and `#[event(queue)]` consumer behind the `queue` feature | `queue.rs`, `event.rs` |
| Durable Objects | `#[durable_object]`, SQLite storage (`State::storage().sql()`), alarms (`set_alarm`, `get_alarm`), hibernatable WebSockets (`accept_web_socket`, tags, auto response) | `durable.rs` lines 281 to 695 |
| Containers from a DO | `State::container()` gives `start(options)`, `running`, `wait_for_exit`, `destroy`, `signal`, `get_tcp_port(port)` (a `Fetcher`); options have `set_entrypoint`, `add_env`, `enable_internet` | `container.rs`; the repo's own test suite has a container (`test/container-echo`) |
| Container outbound intercepts | **Missing.** `interceptOutboundHttp(s)` and `ctx.exports` are not bound | Issue #1072 open. [Docs](https://developers.cloudflare.com/containers/configuration/outbound-traffic/): with `enableInternet: false` only intercepted HTTP(S) leaves the container |
| Workflows | **Missing** | Issue #663 open, PR #918 unmerged. Not in our design, so not a blocker |
| Cache API | `Cache::open`, `put`, `get`, `delete` with string or request keys | `cache.rs` |
| Cron | `#[event(scheduled)]` | `event.rs` line 178 |
| axum | `http` feature makes `fetch` take `http::Request` and return `http::Response`; `axum::Router` is called as a `tower::Service` | `examples/axum`. DO `fetch` does not use `http` types yet (issue #582), so DO handlers map requests by hand |
| Rate limiting binding | `RateLimiter::limit(key)` | `rate_limit.rs` |
| Secrets Store | `SecretStore` binding | `secret_store.rs`; this settles the `[unverified]` in architecture 3.4 |
| Sending email | `env.send_email(name)` with a structured builder or raw MIME | `email.rs`, `examples/send-email` |
| Panics | `worker-build --panic-unwind` (nightly, `-Zbuild-std`) turns panics into JS exceptions and re-inits the instance after an abort | README "Panic Recovery". Without it a panic kills the isolate |
| tokio | Not on `wasm32-unknown-unknown`. An experimental `--emscripten` target with a tokio loop exists | README "Emscripten"; not for v1 |

Size and startup, measured: a release `cdylib` with `worker` (d1, http, axum, queue),
axum, serde_json, jsonwebtoken (RS256, `rust_crypto`), jiff, uuid, chacha20poly1305, hmac
and sha2, `opt-level = "z"`, LTO, one codegen unit: **2.0 MB** raw `.wasm` before
wasm-bindgen and wasm-opt, 560 KB gzipped. The limit is 64 MiB uncompressed and startup
must finish in 1 s ([limits](https://developers.cloudflare.com/workers/platform/limits/)).
Size is not a concern at this scale; startup time of a 3 to 6 MB module `[unverified,
measure with wrangler deploy once the edge crate exists]`.

Gaps that force JS (or a decision):

1. **Outbound intercepts for the build container.** To restrict the builder's egress to
   the source host and our API (architecture 3.8), the DO must register intercepts. No
   Rust API. Options: a tiny JS wrapper module that re-exports the Rust DO and adds the
   intercept calls, contribute the binding upstream (issue #1072), or start with
   `enable_internet(true)` and enforce egress inside `ink build` (weaker, the container is
   our code but the content is not). Pick: ship v1 with internet on and a build-scoped
   token, contribute the binding, and do not ship a JS shim unless the upstream work
   stalls.
2. **Container CA.** Intercepted HTTPS is re-signed by a Cloudflare CA at
   `/etc/cloudflare/certs/cloudflare-containers-ca.crt`. If the builder uses rustls with
   bundled `webpki-roots`, it will reject intercepted traffic. Load that file as an extra
   root when present.
3. **DO with `http` types.** Not a JS shim, just glue code until #582 lands.

Pick: **`worker` 0.9 with `d1`, `http`, `axum`, `queue`**, built with `worker-build
--release`. Add `--panic-unwind` once the nightly requirement is acceptable in CI.

### 1a. R2 presigned uploads

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `rusty-s3` | 0.12.0 | 2026-11-18 | BSD-2-Clause | Compiles | Sans-I/O SigV4 presigning; `sign_with_time(expires, &Timestamp)` takes the time, so it never reads the clock on wasm | Small API surface we must check against R2's S3 quirks `[unverified on R2]` |
| `aws-sigv4` | 1.6.6 | 2026-11-17 | Apache-2.0 | Not checked | Official AWS signer | Pulls parts of the AWS SDK runtime |

Pick: **`rusty-s3`**, time passed in from the `Clock` port.

## 2. One database layer for D1 and SQLite

Constraints found: D1 has no interactive transactions, only atomic `batch`; D1 always
enforces foreign keys and only allows `PRAGMA defer_foreign_keys` inside a statement's
implicit transaction ([foreign keys](https://developers.cloudflare.com/d1/sql-api/foreign-keys/));
D1 binds go through `JsValue`. So the port has to be shaped like D1, and SQLite can always
emulate that shape. The reverse is not true.

| Option | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| Raw SQL behind our own `Db` port | n/a | n/a | n/a | Yes | Port is `query(sql, params) -> Rows` plus `batch(Vec<Stmt>) -> Vec<Rows>` (atomic). D1 adapter uses `prepare`/`bind`/`batch`; SQLite adapter wraps `batch` in one transaction | We write the row mapping. Mitigation: a test that runs every query in `queries.rs` through `EXPLAIN` on SQLite, which catches typos and missing columns |
| `sea-query` | 1.0.2 | 2026-08-12 | MIT OR Apache-2.0 | Compiles (`backend-sqlite`) | Builds `(sql, Values)` for SQLite; a D1 binder (Value to JsValue) is about 40 lines | Builder ceremony for queries that are fixed strings anyway. No D1 binder exists; `sea-query-binder` (0.7.0, 2024-10) is the old crate, the live ones are `sea-query-sqlx` 0.9.1 and `sea-query-rusqlite` 0.8.0 |
| `sqlx-d1` | 0.5.0 | 2026-11-12 | MIT | Compiles | sqlx `Executor` over D1 with compile-time checked `query!` against the local miniflare D1 file | Pins `sqlx-core =0.8.6` while `sqlx` is at 0.9.1, so it cannot share a sqlx version with the native side; pulls `libsqlite3-sys` 0.30 (RUSTSEC-2026-0459) at macro time; no transactions; one maintainer, 54 stars |
| `sqlx-cloudflare-d1` | 0.2.1 | 2026-11-19 | `[unverified]` | Not checked | An `sqlx::Database` over D1 | 59 downloads; too new to judge |
| `diesel` | 2.4.0 | 2026-11-13 | MIT OR Apache-2.0 | Not checked | No D1 backend; `diesel-d1` 0.1.0 has 3 downloads (2024-12) | Sync API, not a fit for D1 |
| `libsql` | 0.10.0 | 2026-10-13 | MIT | Not checked | Turso client (embedded or remote over HTTP) | Talks to libSQL servers, not to D1 |
| `turso` (was limbo) | 0.8.4 | 2026-11-20 | MIT | Not checked | SQLite rewrite in Rust | Pre-1.0 engine; a self-host database file is user data, so not there yet |

How other Rust Workers projects do it: the workers-rs README and examples use `prepare`,
`bind` and serde `results::<T>()` directly. `sqlx-d1` is the only attempt at a shared
query API with real use, and its version pin is the problem above.

Migrations: wrangler applies every `*.sql` in `migrations/` in filename order and records
them in `d1_migrations(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE, applied_at
TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL)` (read in workers-sdk
`packages/wrangler/src/d1/migrations/helpers.ts`).

| Option | Version | Last release | License | Fit | Risk |
|---|---|---|---|---|---|
| Own runner over the same files and the same `d1_migrations` table | n/a | n/a | n/a | About 60 lines: `include_dir!` or `include_str!` the folder, apply missing names in order inside one SQLite transaction each, insert into `d1_migrations`. Hosted and self-host then have identical history | Must reject statements D1 rejects (`BEGIN`, `COMMIT`) so a file that works self-hosted also works on D1 `[unverified exact D1 rejection list]` |
| `refinery` | 0.10.0 | 2026-10-02 | MIT | Needs `V1__name.sql` names and its own `refinery_schema_history` table | Two naming schemes and two history tables for one folder |
| sqlx migrate | 0.9.1 | 2026-11-18 | MIT OR Apache-2.0 | `0001_name.sql` naming matches wrangler | Only if we use sqlx natively; `_sqlx_migrations` table with checksums differs from D1's |
| `rusqlite_migration` | 2.9.0 | 2026-11-23 | Apache-2.0 | Uses `user_version`, migrations as Rust strings | Not a folder wrangler can read |

Pick: **raw SQL behind our own `Db` port, D1-shaped (no interactive transactions, atomic
batch)**, and **our own migration runner over `migrations/NNNN_name.sql` writing the same
`d1_migrations` table**. sea-query only if dynamic filters show up later.

**Revised by 0049 (typed statements).** The owner rule is no SQL text, so the port holds typed
builders. `sea-query` 1.0.2 (MIT OR Apache-2.0; read from its source) has `Table::create()`,
`Index::create()`, `ForeignKey`, column `check`, `auto_increment`, `ON CONFLICT ... DO UPDATE`,
`RETURNING`, `INSERT ... SELECT` and scalar subqueries, renders the SQLite dialect and binds
values as `?`; with `default-features = false` and `derive` plus `backend-sqlite` it has no
driver and no async runtime, so `db` stays free of both. Its `Value` enum maps onto D1's five
kinds in one `TryFrom`. Diesel was not tried: its query types come from `table!` macros and run
through its own `Connection`, which fits a port that takes values and a renderer less well.
What the typed form costs: the builders take `&mut self`, so a statement ends in `.to_owned()`,
and `INSERT ... SELECT` returns a `Result` for a column count mismatch, which `control` carries
as `Error::Statement`. wrangler can still apply a D1 migration from a `.sql` file rendered from
the same values, but nothing does yet.

## 3. Self-host stack

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `axum` | 0.8.9 | 2026-04-14 | MIT | Compiles (no default features) | Same router type on both sides: the Worker calls it as a `tower::Service`, `ink serve` runs it on hyper | Handler state must be `Send + Sync`; Worker bindings are `!Send`, so the hosted adapter wraps them (workers-rs provides `send::SendWrapper`) |
| `tower-http` | 0.7.1 | 2026-08-31 | MIT | Not checked | Compression, timeouts, trace, limits for `ink serve` | axum 0.8.9 itself is tested against 0.6.8; 0.7 works with tower 0.5 and http 1, so mixing is fine |
| `tokio` | 1.54.1 | 2026-10-21 | MIT | No (by design) | Native only, in `uses-edge-std` | None |
| `rusqlite` | 0.43.0 | 2026-11-15 | MIT | No | Sync, `bundled` SQLite via `libsqlite3-sys` 0.40 (past the RUSTSEC-2026-0459 fix). One writer connection on a blocking thread fits the `batch` port exactly | Bundled C SQLite means a C compiler in cross builds |
| `sqlx` (sqlite) | 0.9.1 | 2026-11-18 | MIT OR Apache-2.0 | No | Async pool, compile-time checks | The checks do not reach the D1 side, so they cover half the code; heavier |
| `rustls` | 0.23.46 | 2026-10-14 | Apache-2.0 OR ISC OR MIT | n/a | TLS for `ink serve --tls acme` | Default provider is `aws-lc-rs` (needs cmake and a C toolchain when cross-compiling); the `ring` provider is simpler for musl |
| `instant-acme` | 0.8.5 | 2026-02-24 | Apache-2.0 | n/a | ACME client only (http-01, dns-01, tls-alpn-01). We drive issuance per hostname from the domains table and serve certs from our own `ResolvesServerCert` | We write the on-demand logic, renewal and the cert cache (blob directory) |
| `rustls-acme` | 0.15.4 | 2026-08-11 | Apache-2.0 OR MIT | n/a | Turnkey tls-alpn-01 and http-01 with axum integration | Domain list is fixed at `AcmeConfig::new(domains)`; adding a custom domain means a new state per domain. Repo last pushed 2026-07 |
| `tokio-rustls-acme` | 0.11.0 | 2026-11-26 | Apache-2.0 OR MIT | n/a | n0's fork of the above | Same fixed-list model `[unverified]` |

Pick: **axum 0.8 + tower-http 0.7 + tokio + rusqlite (bundled)**, and **rustls with the
`ring` provider plus `instant-acme`** driving on-demand issuance limited to hostnames in
the domains table, as architecture 3.7 requires. rustls-acme is the fallback if on-demand
proves not worth it.

## 4. Forge auth, webhooks, and one HTTP client

### 4a. The `Http` port

reqwest 0.13 compiles for wasm32 and its wasm client calls the global `fetch`
(`src/wasm/client.rs`, `js_fetch`), so it can run in a Worker `[unverified at runtime in
workerd]`. It still hides things we want visible: subrequest counts, `cf` request options,
and timeouts (on wasm, reqwest has none). Proposed port:

```rust
trait Http {
    async fn send(&self, req: http::Request<Bytes>) -> Result<http::Response<Bytes>, HttpFault>;
}
```

Worker impl: `worker::Fetch` with an `AbortSignal` timeout. Native impl: reqwest with
rustls. No `Send` bound on the future (native callers that need `Send` use a wrapper).
Every client library below is either given this port or replaced by a few typed calls
over it.

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `reqwest` | 0.13.5 | 2026-09-08 | MIT OR Apache-2.0 | Compiles | Native impl of the port | None for native |
| `http` | 1.5.0 | 2026-07-29 | MIT OR Apache-2.0 | Yes | Port request and response types, shared with axum and workers-rs | None |

### 4b. OAuth, GitHub App, forges

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `oauth2` | 5.0.0 | 2025-01-21 | MIT OR Apache-2.0 | Compiles (no default features) | Typed PKCE, CSRF state, token responses; takes any `AsyncHttpClient` (a closure over our port works) | Last release 2025-01; depends on `chrono` |
| `jsonwebtoken` | 12.0.0 | 2026-11-13 | MIT | Compiles with `rust_crypto` | RS256 for the GitHub App JWT; uses `rsa` 0.10 (past the Marvin fix) | Must pick a crypto backend explicitly; `aws_lc_rs` will not build for wasm |
| `jwt-simple` | 0.15.1 | 2026-11-18 | ISC | Compiles with `pure-rust` | Alternative JWT crate | No reason to switch |
| `octocrab` | 0.54.9 | 2026-10-31 | MIT OR Apache-2.0 | Compiles only with no default features, which removes its hyper client | Models are usable; the client is not, on wasm | Large dependency for about six endpoints we call |
| `forgejo-api` | 0.12.0 | 2026-10-15 | Apache-2.0 OR MIT | **Fails** (hard `tokio` `net` dependency, mio) | Native only | Cannot be shared |
| `gitea-sdk` | 0.6.3 | 2026-08-04 | MIT | Not checked | Small, 20k downloads | reqwest-bound `[unverified]` |
| `gitlab` | 0.1905.0 | 2026-10-23 | MIT/Apache-2.0 | Not checked | Full GitLab API | Sync-first, huge; not for wasm |

Endpoints we need are few per forge (current user, list repos, repo permissions, create and
delete a webhook, installation token for GitHub). Typed request and response structs over
the `Http` port are smaller than any of these clients and identical on both targets.

### 4c. Webhook signatures

| Forge | Header | Scheme |
|---|---|---|
| GitHub | `X-Hub-Signature-256` | `sha256=` hex HMAC-SHA256 of the body |
| Forgejo / Gitea | `X-Forgejo-Signature`, `X-Gitea-Signature` | hex HMAC-SHA256 of the body (architecture 3.4) |
| GitLab 19.1+ | `webhook-id`, `webhook-timestamp`, `webhook-signature` | Standard Webhooks: `v1,` base64 HMAC-SHA256 over `{id}.{timestamp}.{body}`, key is the `whsec_` token base64-decoded ([GitLab docs](https://docs.gitlab.com/user/project/integrations/webhooks/), GA in 19.1). Older instances: plain `X-Gitlab-Token` |

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `hmac` | 0.13.0 | 2026-03-29 | MIT OR Apache-2.0 | Compiles | `Mac::verify_slice` is constant time | 0.13 moved `new_from_slice` to `KeyInit` (API churn from 0.12) |
| `sha2` | 0.11.2 | 2026-11-25 | MIT OR Apache-2.0 | Compiles | | |
| `subtle` | 2.6.1 | 2024-06-24 | BSD-3-Clause | Compiles | `ConstantTimeEq` for token compares (GitLab legacy token, session hashes) | None |
| `standardwebhooks` | 1.3.0 | 2026-11-17 | MIT | Not checked | Reference verifier for the GitLab scheme | Ten lines with `hmac`; not worth a dependency |

Pick: **own typed forge clients over the `Http` port; `oauth2` for the auth-code + PKCE
flow; `jsonwebtoken` with `rust_crypto` for the GitHub App JWT; `hmac` + `sha2` + `subtle`
for webhook checks**, one verifier per forge scheme.

## 5. Sessions, passkeys, email

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `cookie` | 0.18.3 | 2026-11-23 | MIT OR Apache-2.0 | Compiles (`signed`, `private`) | Parse and build `Set-Cookie`; signing not needed if the cookie is an opaque id | None |
| `tower-sessions` | 0.15.0 | 2026-02-01 | MIT | Not checked | axum-only session middleware | Needs `Send` stores and a tokio-style runtime for expiry `[unverified on wasm]`; our sessions are rows anyway |
| `webauthn-rs` | 0.6.2 | 2026-10-31 | MPL-2.0 | **Compiles** (needs `uuid` `js` feature); 0.6 replaced OpenSSL with `crypto-glue` over RustCrypto | Relying party for passkeys on both targets | MPL-2.0 (file-level copyleft, fine for us unmodified); runtime in workerd `[unverified]` |
| `passkey` | 0.6.0 | 2026-10-01 | MIT OR Apache-2.0 | Not checked | 1Password's client and authenticator side | Not a relying party; wrong half |
| `lettre` | 0.11.24 | 2026-11-17 | MIT | No | SMTP for self-host | None |
| `mail-builder` | 2.0.1 | 2026-11-02 | Apache-2.0 OR MIT | Not checked | Raw MIME for the Worker `send_email` raw path | Only needed for custom headers |
| `resend-rs` | 0.38.0 | 2026-11-23 | MIT | Not checked | Resend client | reqwest-bound `[unverified]`; plain REST over the port is simpler |
| `postmark` | 2.0.1 | 2026-05-28 | MIT OR Apache-2.0 | Not checked | Postmark client | Same |
| `askama` | 0.16.1 | 2026-09-04 | MIT OR Apache-2.0 | Pure codegen `[unverified build on wasm]` | Compile-time checked email templates (text and HTML) | None |

Email from Workers: Cloudflare Email Service with a `send_email` binding is in workers-rs
0.9 (`examples/send-email`). On Workers Paid with an onboarded sending domain it sends to
any recipient; 3,000 emails a month included, then $0.35 per 1,000; new accounts start
with a low daily quota that grows ([limits](https://developers.cloudflare.com/email-service/platform/limits/),
[pricing](https://developers.cloudflare.com/email-service/platform/pricing/)).
`wrangler dev` writes `.eml` files instead of sending.

Session design that needs no session crate: 32 random bytes in a `__Host-session`
cookie (`Secure`, `HttpOnly`, `SameSite=Lax`, path `/`), the SHA-256 of it as the key of a
`session` row with expiry and identity. Magic links use the same shape (single use, 15
minutes, row deleted on use). CSRF: `SameSite=Lax` plus an `Origin` check on every
state-changing request.

Pick: **opaque session rows + `cookie`; Cloudflare Email Service hosted and `lettre`
self-hosted behind a `Mailer` port; `askama` templates; `webauthn-rs` 0.6 for passkeys,
after v1** (email and forge sign-in cover launch).

## 6. ATproto

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `atrium-api` | 0.26.0 | 2026-10-27 | MIT | Compiles | Generated lexicon types | Depends on `chrono` (unmaintained) |
| `atrium-identity` | 0.2.0 | 2026-10-27 | MIT | Compiles | DID resolution (PLC, did:web), handle resolution (DNS TXT, DoH TXT, `.well-known`), generic over an HTTP client trait, so our port fits | Small team (418 stars) |
| `atrium-oauth` | 0.2.0 | 2026-10-27 | MIT | Compiles | atproto OAuth client with DPoP and PAR | Pulls `rsa` 0.9 (RUSTSEC-2023-0071) and `elliptic-curve` 0.13 with `crypto-bigint` 0.5 (RUSTSEC-2026-0387) through `jose-jwk` |
| `jacquard-oauth`, `-identity`, `-common` | 0.12.2 | 2026-09-05 | MPL-2.0 | Compiles | Newer design, same features | Hard dependency on `reqwest` 0.12 (we use 0.13, so two copies); the same `jose-jwk` advisories; repo on tangled.org, 5k downloads |

Pick: **`atrium-identity` for DID and handle resolution, `atrium-oauth` for sign-in**, with
the two advisories listed as reviewed exceptions in `deny.toml` (we never do RSA private
key operations through that path; ES256 is ours). If the exceptions are unacceptable, the
fallback is our own DPoP + PAR + PKCE client on current `p256`, about 400 lines
`[unverified estimate]`.

## 7. DNS TXT verification

Cloudflare's DoH JSON API answers `GET https://cloudflare-dns.com/dns-query?name=<n>&type=TXT`
with `accept: application/dns-json` (checked: `_atproto.bsky.app` returns the DID record
as a quoted string in `Answer[].data`).

| Option | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| DoH JSON over the `Http` port, both targets | n/a | n/a | n/a | Yes | One implementation, one behavior hosted and self-hosted; about 50 lines with typed `Answer` | Self-hosters behind restrictive egress need the resolver URL configurable |
| `hickory-resolver` | 0.26.4 | 2026-11-24 | MIT OR Apache-2.0 | No (sockets) | System or custom resolver natively | A second code path that behaves differently (caching, search domains) |
| `atrium-identity` DoH resolver | 0.2.0 | 2026-10-27 | MIT | Compiles | Already in the tree for ATproto; uses `hickory-proto` wire format | Built for handle lookups, not general TXT |

Pick: **DoH JSON over the `Http` port on both targets**, resolver URL in config with
`https://cloudflare-dns.com/dns-query` as the announced default. Query with `cd=false` and
treat `Status != 0` as a typed failure, not as "record absent".

## 8. Sealing secrets at rest

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `chacha20poly1305` | 0.11.0 | 2026-06-28 | Apache-2.0 OR MIT | Compiles | `XChaCha20Poly1305`: 24-byte random nonce, no nonce counter to keep, AAD = row id | None |
| `aes-gcm` | 0.11.1 | 2026-08-21 | Apache-2.0 OR MIT | Compiles | AES-256-GCM as written in architecture 3.4 | 12-byte random nonces have a birthday bound; fine at our volume but one more thing to argue about |
| `age` | 0.12.1 | 2026-07-14 | MIT OR Apache-2.0 | Compiles alone | Good for exports and operator backups (recipient keys) | **Cannot share a build graph with `hpke` 0.15**: age pulls `ml-kem` 0.2 which needs `kem 0.3.0-pre.0`, hpke pulls `kem` 0.3.0, and together age fails to compile (checked, natively and on wasm) |
| `hpke` | 0.15.2 | 2026-11-26 | MIT/Apache-2.0 | Compiles | Public-key sealing | Not needed: we seal to ourselves, a symmetric key is enough |

Key source: hosted, a Secrets Store binding (`SecretStore` in workers-rs 0.9) or a Worker
secret; self-host, the `0600` key file from architecture 3.4. Envelope: one version byte,
a key id byte (so rotation can keep old keys for opening), nonce, ciphertext.

Pick: **`chacha20poly1305` (XChaCha20-Poly1305), AAD = owning row id, key from the
`Secrets` port**. Amend architecture 3.4 from AES-256-GCM. No `age` and no `hpke` in the
control plane.

## 9. Cloudflare for SaaS custom hostnames

| Option | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `cloudflare` (cloudflare-rs) | 0.14.0 | 2025-03-13 | BSD-3-Clause | Compiles (no default features) | Has `dns`, `zones`, `r2`, `workers` endpoints | **No custom hostnames endpoint** (`src/endpoints/` has none); last release 2025-03, 69 open issues |
| Plain REST over the `Http` port | n/a | n/a | n/a | Yes | `POST/GET/PATCH/DELETE /zones/{zone}/custom_hostnames[/{id}]`, typed `status` and `ssl.status` enums mapped one to one to domain states | Cloudflare notes the create response may omit `validation_records`; fetch again after a delay ([getting started](https://developers.cloudflare.com/cloudflare-for-platforms/cloudflare-for-saas/start/getting-started/)) |

Pick: **plain REST with our own typed structs**, API token as a Worker secret scoped to the
one zone.

## 10. IDs, time, rate limits, serde, errors

| Crate | Version | Last release | License | wasm32 | Fit | Risk |
|---|---|---|---|---|---|---|
| `uuid` | 1.29.3 | 2026-11-17 | Apache-2.0 OR MIT | Compiles (`v7`, `js`) | RFC 9562 v7, time-ordered, standard everywhere | Must enable `js` on wasm or it fails to compile (seen with webauthn-rs) |
| `ulid` | 3.0.0 | 2026-07-16 | MIT | Compiles | 26-char sortable ids | Less standard; no advantage over v7 |
| `jiff` | 1.0.3 | 2026-11-24 | Unlicense OR MIT | Compiles (`js`) | 1.0 is out; `Timestamp` for facts, the `Clock` port returns one | None. `chrono` is now flagged unmaintained (RUSTSEC-2026-0543) and still arrives through `worker`, `oauth2`, `atrium-api`, `octocrab`, `cloudflare`; keep it out of our own types |
| Workers rate limiting binding | in `worker` 0.9 | | | Yes | `limit(key)`; per-location and approximate, period 10 or 60 s ([docs](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)) | Not exact: fine for abuse throttling, wrong for budgets users see |
| `governor` | 0.10.4 | 2025-12-16 | MIT | Compiles | In-memory GCRA for `ink serve` | In a Worker it would be per isolate, so useless there |
| `serde`, `serde_json` | 1.0.229 | 2026-07-18 | MIT OR Apache-2.0 | Yes | | None |
| `thiserror` | 2.0.21 | 2026-09-23 | MIT OR Apache-2.0 | Yes | Derives `Display`, `Error`, `From` on our own enums | It is a derive, not a design; the typed `Diagnostic` stays ours |

Budget limits that users see (30 builds per handle per hour, architecture 3.8) are counted
in rows (or in the per-handle build DO), so they are exact and the same code runs
self-hosted.

Pick: **`uuid` v7 behind per-entity newtypes, stored as TEXT; `jiff`; the Workers rate
limit binding and `governor` behind one `Throttle` port for abuse only; budgets as rows;
`serde`; own error enums with `thiserror`.**

## 11. Release and ops

| Tool | Version | Last release | License | Fit | Risk |
|---|---|---|---|---|---|
| `cargo-dist` | 0.34.0 | 2026-11-02 | MIT OR Apache-2.0 | Generates the GitHub Actions release workflow, archives, shell and PowerShell installers, checksums for `ink` | 341 open issues; adopt its generated workflow, do not hand-edit it |
| `cargo-deny` | 0.22.1 | 2026-11-24 | MIT OR Apache-2.0 | Advisories, licenses, bans (the plugin I/O ban list from architecture 3.2) | Already flags: RUSTSEC-2026-0543 (chrono), RUSTSEC-2023-0071 and RUSTSEC-2026-0387 (via the ATproto crates), RUSTSEC-2026-0459 if `sqlx-d1` were used |
| `cargo-auditable` | 0.7.8 | 2026-11-25 | MIT OR Apache-2.0 | Embeds the dependency list in the binary | None |
| wrangler | `[unverified version]` | | | `wrangler deploy` builds the Worker through `worker-build`, builds the container image from a Dockerfile and pushes it to Cloudflare's registry | Needs Docker on the CI runner (GitHub's Ubuntu runners have it) |

Container image (Cloudflare requires `linux/amd64`, [image docs](https://developers.cloudflare.com/containers/get-started/)):
build `ink` for `x86_64-unknown-linux-musl`, static, and copy it into a static base.
Compressed layer sizes read with `skopeo` on 2026-11-26:

| Base | Compressed size | Notes |
|---|---|---|
| `cgr.dev/chainguard/static:latest` | 460 KB | CA bundle, `nonroot` user, tzdata; only `latest` is free |
| `gcr.io/distroless/static-debian13:nonroot` | 860 KB | Same contents, Debian-tracked tags |
| `gcr.io/distroless/static-debian12:nonroot` | 716 KB | Older Debian |
| `gcr.io/distroless/cc-debian13:nonroot` | 10.7 MB | Only if a glibc binary is needed |

Cold start is documented as "often 1 to 3 seconds, depending on image size"
([FAQ](https://developers.cloudflare.com/containers/faq/)), so the binary dominates, not the
base.

Cross-compiling: musl static needs a C compiler for bundled SQLite and for the TLS
provider. `ring` builds with `cc`; `aws-lc-rs` also wants cmake. cargo-dist can build musl
targets; `cargo-zigbuild` is the usual fallback `[unverified cargo-dist musl handling for
C dependencies]`.

CI on GitHub Actions: one workflow with `cargo fmt`, `clippy`, `cargo deny check`,
`cargo test` (native), `cargo check --target wasm32-unknown-unknown -p uses-edge-cf`, the
miniflare suite (section 12), then on `main` a deploy job running `wrangler deploy` with a
scoped `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`. Releases of `ink` come from the
cargo-dist workflow on tags.

Pick: **cargo-dist for `ink`, `chainguard/static` (or distroless static-debian13 if we
want pinned tags) with a musl static binary, cargo-deny with a reviewed exceptions list,
wrangler deploy from CI.**

## 12. Testing

| Layer | Tool | Notes |
|---|---|---|
| Control plane logic | `cargo test` with in-memory ports (`MemBlobs`, `MemQueue`, `FakeClock`, `FakeHttp` with recorded responses) and **real SQLite in memory** for `Db` | SQLite is what D1 runs, so the same SQL and migrations are exercised; `FakeHttp` replays forge, DoH and Cloudflare API fixtures |
| `Db` port contract | One test suite, run against the rusqlite adapter natively and against D1 through miniflare | Catches D1-only differences (foreign keys, no `BEGIN`, JsValue number conversions) |
| Worker adapter | vitest + miniflare against the built Worker, the way workers-rs tests itself (`test/`, `miniflare` 5.20261110.1-beta, `vitest` 5, `worker-build --dev` first) | Covers D1, R2, Queues, DO alarms, cache. The README's "Testing with Miniflare" section describes the setup |
| Container start | `wrangler dev` runs containers locally through Docker; `--enable-containers=false` turns it off | Slow; run in a separate CI job |
| Pure-Rust tests inside wasm | `wasm-bindgen-test` 0.3.80 | Only for small checks (uuid, getrandom, jiff `now`) that must hold on wasm |
| End to end | `ink serve` in CI with a throwaway state dir and fixture repos | Same control plane, no Cloudflare account needed |

Pick: **in-memory ports + SQLite for most tests, a shared `Db` contract suite on both
adapters, a small vitest + miniflare suite for the Worker adapter.**

## Pick list

| Need | Pick |
|---|---|
| Worker runtime | `worker` 0.9 (`d1`, `http`, `axum`, `queue`), `worker-build` |
| R2 presigned PUTs | `rusty-s3` 0.12 with time from `Clock` |
| Database | Own D1-shaped `Db` port (query + atomic batch), raw SQL, `rusqlite` 0.43 bundled natively |
| Migrations | `migrations/NNNN_name.sql`, wrangler hosted, own runner self-hosted, both writing `d1_migrations` |
| Self-host HTTP | `axum` 0.8, `tower-http` 0.7, `tokio` |
| Self-host TLS | `rustls` 0.23 (`ring` provider) + `instant-acme` 0.8, on-demand for known hostnames |
| HTTP client | Own `Http` port over `http` types: `worker::Fetch` hosted, `reqwest` 0.13 native |
| OAuth | `oauth2` 5 with our HTTP closure |
| GitHub App JWT | `jsonwebtoken` 12 with `rust_crypto` |
| Forge APIs | Own typed calls for GitHub, Forgejo/Gitea, GitLab |
| Webhooks | `hmac` 0.13 + `sha2` 0.11 + `subtle`; GitHub, Forgejo/Gitea, GitLab Standard Webhooks |
| Sessions | Opaque token cookie (`cookie` 0.18), hashed rows |
| Passkeys | `webauthn-rs` 0.6, after v1 |
| Email | Cloudflare Email Service binding hosted, `lettre` self-hosted, `askama` templates |
| ATproto | `atrium-identity` + `atrium-oauth` 0.2, advisories as reviewed exceptions |
| DNS TXT | DoH JSON over the `Http` port, both targets |
| Sealing | `chacha20poly1305` (XChaCha20-Poly1305), key from `Secrets` |
| Custom hostnames | Plain REST to the Cloudflare API |
| IDs and time | `uuid` v7 newtypes, `jiff` 1 |
| Rate limits | Workers binding + `governor` for abuse; budgets as rows |
| Errors | Own enums, `thiserror` 2 |
| Release | `cargo-dist`, musl static, `cgr.dev/chainguard/static`, `cargo-deny`, `cargo-auditable` |
| Tests | In-memory ports + SQLite, shared `Db` contract suite, vitest + miniflare for the Worker |

## Gaps we must build ourselves

1. The `Db` port, the D1 adapter (JsValue binding and row decoding) and the rusqlite
   adapter, plus the `EXPLAIN` check over every query.
2. The migration runner that reads the wrangler folder and writes `d1_migrations`, and a
   lint that rejects `BEGIN`/`COMMIT` in migration files.
3. The `Http` port with both impls, timeouts and a subrequest counter on the Worker side.
4. Typed clients for GitHub (installation tokens, repos, hooks), Forgejo/Gitea (user,
   repos, hooks, per-instance OAuth config) and GitLab (user, projects, hooks).
5. Webhook verifiers for three schemes, including Standard Webhooks timestamps and
   replay window.
6. Session and magic-link rows, `Origin` check, the `Mailer` port with two impls.
7. DoH TXT lookup with typed failure modes.
8. The sealing envelope (version, key id, nonce, AAD) and key rotation.
9. Cloudflare custom hostname client with the state mapping and the delayed re-fetch.
10. On-demand ACME for `ink serve`: issuance queue, cert cache in the blob directory,
    renewal, SNI resolver limited to the domains table.
11. Container outbound intercepts: either the workers-rs binding (upstream PR) or, if that
    stalls, a small JS module wrapping the Rust DO. Plus loading the Cloudflare containers
    CA in the builder.
12. `http`-typed request mapping for Durable Object `fetch` until workers-rs #582 lands.
13. A `Throttle` port over the Workers rate limit binding and `governor`.
