# Spike: workers-rs Durable Object, Container, R2

M0 spike 3 (`docs/roadmap.md`). Question: can a workers-rs 0.9 Durable Object start a Container,
stream its output to an HTTP client as it arrives, write the log to R2, and restrict the
container's outbound network, given that workers-rs binds no outbound intercepts (#1072)?

**Verdict: yes, in Rust, with no JS shim.** Everything workers-rs 0.9 lacks (`ctx.exports`,
`interceptOutboundHttp`, `exec`) is reachable from Rust through `State::as_raw()`,
`Container: AsRef<JsValue>` and `js_sys::Reflect`. The outbound handler is the worker's own
`#[event(fetch)]`, reached as `ctx.exports.default({ props })`, and told apart from public traffic
by its props. DO streaming and R2 ran locally. **The container itself never started locally**:
`wrangler dev` on this host hits an open workers-sdk bug (below), so container start, the
container-to-DO stream and egress enforcement are built but unverified.

## What is here

| Path | What |
|---|---|
| `src/lib.rs` | The worker: routes, the `Build` Durable Object, the egress handler |
| `wrangler.toml` | DO binding, SQLite migration, `[[containers]]`, R2 binding; local only |
| `container/` | `emit`, a std-only Rust binary standing in for `ink build`, on `alpine:3` (14.7 MB image) |

Routes:

| Route | Does |
|---|---|
| `POST /build/<name>/run?lines=&interval=&internet=&intercept=` | start the container if needed, stream `GET /run` from its port 8080, write the log to R2 |
| `POST /build/<name>/exec?lines=&interval=` | stream real stdout of `container.exec(["/emit","lines",..])` |
| `POST /build/<name>/egress?host=` | ask the container to `GET http://<host>/` and report each step |
| `POST /build/<name>/local?lines=&interval=` | an in-DO producer instead of a container: tests DO streaming and R2 alone |
| `POST /build/<name>/handler?host=` | call the egress handler the way the sidecar would, without a container |
| `POST /build/<name>/stop` | `container.destroy()` |
| `GET /log/<key>` | read a log back from R2 |

Every response line that is not output starts with `# ` and says what happened (intercepts
registered, start time, R2 key, byte and chunk counts, or the failure).

## Method

- Versions read on 2026-11-26: `worker` 0.9.0 (crates.io, released 2026-11-11, still latest),
  `worker-build` 0.9.0, clone at `~/code/strangers/workers-rs` commit `a61be78` (2026-11-17, the
  current `main` head per the GitHub API), wrangler 4.168.0 with workerd 1.20261123.1, Docker
  29.9.2 (rootful, native), Fedora, kernel 7.2.
- Build: `worker-build --release` (wrangler runs it). Run: `npx wrangler@4.168.0 dev`.
- Latency: each line carries `t=<epoch ms>` from its producer; the client stamps arrival with
  `date +%s%3N` on the same host:
  `curl -sN -X POST 'http://127.0.0.1:8787/build/a/local?lines=10&interval=200' | while IFS= read -r l; do echo "$(date +%s%3N) $l"; done`
- A pure-JS control worker (one DO, `ctx.container.start()`, same image) separated workers-rs
  problems from local-runtime problems.

## What ran

| Check | Result |
|---|---|
| Worker compiles with `worker` 0.9.0, edition 2024, Rust 1.100 | yes, clippy clean |
| Release build | `index_bg.wasm` 533 KB, 177 KB gzipped; `index.js` 30 KB |
| DO streams lines to the client as produced (`/local`) | yes, each line arrives on its own, not buffered |
| R2 put of the collected log at stream end, read back through `GET /log/..` | yes (miniflare R2), 230 bytes in 6 ms |
| `ctx.exports.default({ props })` from Rust | yes, after one fix (gotcha 1) |
| Egress handler via that loopback (`/handler`) | `denied.test` 403, `allowed.test` 200 answered by the worker, `example.com` 200 forwarded |
| Public request cannot reach the egress branch | `Host: denied.test` from outside gets the public 404 |
| `container.interceptOutboundHttp(host, loopback)` from Rust | accepted for 3 hosts before `start()` |
| `container.start(ContainerStartupOptions)` with `enable_internet(false)` | returns (options validate) |
| `emit` image under plain `docker run` | starts, serves `/ping`, streams `/run` |

## What did not run

The workload container never started under `wrangler dev`, for the Rust worker and for the
pure-JS control alike. workerd creates and starts the `cloudflare/proxy-everything` networking
sidecar, then never asks Docker to create the workload container; after about 30 s it fails with
`kj/timer.c++:30: overloaded: operation timed out`, and `getTcpPort(8080).fetch()` reports
`connect(): Container ingress proxy is not running.` The sidecar's ingress accepts TCP but never
answers a CONNECT. This is workers-sdk issue
[#15394](https://github.com/cloudflare/workers-sdk/issues/15394), open since 2026-08-28,
reproduced there on native Linux Docker 29 (rootful and rootless) up to wrangler 4.163; one
reporter says it works in a Debian VM. So it is the local runtime, not workers-rs.

Unverified because of it:

- container start time inside workerd, and whether `ContainerStartupOptions` from 0.9 behaves
  (it has an inverted `delete_property` check and passes `env` as a `Map`; PR #1143 calls that
  conversion broken; this spike sets no env);
- the container-to-DO stream over `getTcpPort(8080)` (`/run`) and over `exec` stdout (`/exec`);
- that intercepts actually route the container's traffic, and that `enableInternet: false` blocks
  the rest (`/egress`);
- what happens to the R2 write when the client disconnects mid-stream.

Older wrangler is not a way around it: wrangler 4.127.1 (workerd from 2026-09) cannot run a
workers-rs 0.9 worker at all, it throws `Critical error` from `Tracing::get_active_span`
(`getActiveSpan` is missing in that runtime).

## Numbers

All local, on a laptop, through miniflare and Docker. None of this says anything about hosted
cold start, which Cloudflare documents as often 1 to 3 s depending on image size.

| Measure | Value |
|---|---|
| `docker run` of the `emit` image to first `/ping` answer, 5 runs | 391, 267, 258, 248, 273 ms |
| Container HTTP chunked stream to a client, 20 lines at 100 ms | mean 1.7 ms, max 2 ms behind the producer |
| DO stream to a client through the worker, 10 lines at 200 ms (`/local`) | mean 3 ms, max 8 ms |
| R2 put of the collected log (miniflare) | 6 ms for 230 bytes |
| Egress handler round trip through the loopback | 5 to 16 ms local; 58 ms forwarding to `example.com` |

The DO's `t=` comes from workerd's `Date.now()`, which only advances on I/O, so the DO-side
latency is approximate.

## Outbound network: the answer

**No JS shim, no wait for upstream.** The DO restricts egress like this (all in `src/lib.rs`):

1. Build the handler: `Reflect::apply(ctx.exports.default, ctx.exports, [{ props: { egress: true, build } }])`.
   The default export of a workers-rs worker is a `WorkerEntrypoint` (worker-build's shim), so
   `ctx.exports.default` is its loopback. Needs compatibility date 2025-11-17 or later.
2. For each allowed host, `container.interceptOutboundHttp(host, handler)` (and
   `interceptOutboundHttps` for HTTPS), before `start()`.
3. `start()` with `enableInternet: false`. Per the Cloudflare docs, only intercepted HTTP(S)
   then leaves the container, DNS answers only intercepted names, and other protocols are dropped.
4. In `#[event(fetch)]`, `ctx.props::<EgressProps>()` succeeds only for the loopback, so that
   branch is the egress policy: check the host, forward with `Fetch::Request`, or refuse.

What this gives the builder, beyond an allowlist: the handler runs outside the container and can
attach the forge credential itself (`redirect: "manual"`, check host and scheme first), so `ink
build` never holds the installation token. For HTTPS, the container must trust the ephemeral CA
at `/etc/cloudflare/certs/cloudflare-containers-ca.crt`; with rustls and bundled `webpki-roots`
that means loading the file as an extra root when present.

What workers-rs 0.9 lacks, precisely:

| Missing | Workaround used here | Upstream |
|---|---|---|
| `ctx.exports` on `State` and `Context` | `Reflect::get(state.as_raw(), "exports")` | PR #1078, #1144 |
| calling a loopback factory with props | `Reflect::apply`, never `Function::call1` | #1144 does the same |
| `Container::intercept_outbound_http(s)`, `intercept_all_outbound_http` | `Reflect::get(container, ..)` then `call2` | PR #1078, #1144 |
| `Container::exec` with streamed stdout | `Reflect` call, `ByteStream::from(stdout)` | PR #1143, #1144 |
| named `WorkerEntrypoint` classes in Rust | not needed: the default entrypoint plus props | PR #1079 |
| `setInactivityTimeout` | not used | #1144 |

State of #1072 (read-only, GitHub API, 2026-11-26): open, no comments. Two community
PRs implement it: #1078 (bindings for `exports` and the intercepts, opened 2026-09-27, last
touched 2026-11-15, asks maintainers about the `Exports` design; no maintainer comment on the thread) and #1144
(stacked on #1143, exec streams plus intercepts plus `exported_fetcher`, opened 2026-11-13). Both
are unmerged.

### Draft upstream change

There is no need for a third PR; the useful contribution is a review comment on #1078 or #1144
with what this spike found. Draft:

> We use the intercepts from Rust today through `Reflect` and would like typed bindings. Two
> things we hit that the binding should handle:
>
> 1. `ctx.exports.<name>` is a `LoopbackServiceStub`. Calling it with `Function::call1` makes
>    wasm-bindgen emit `f.call(this, opts)`, and `.call` on an RPC stub is a remote method name,
>    so it fails with `DataCloneError: Could not serialize object of type "LoopbackServiceStub"`.
>    `Reflect::apply` works. #1144 already does this; #1078 should too.
> 2. The default export is enough for many uses: `ctx.exports.default({ props })` with
>    `Context::props()` on the receiving side lets the existing `#[event(fetch)]` be the outbound
>    handler, with no named entrypoint codegen (#1079). Worth a doc example.
>
> Suggested surface:
>
> ```rust
> impl State   { pub fn exports(&self) -> Result<Exports>; }
> impl Context { pub fn exports(&self) -> Result<Exports>; }
> impl Exports { pub fn entrypoint(&self, name: &str, props: Option<&impl Serialize>) -> Result<Fetcher>; }
> impl Container {
>     pub async fn intercept_outbound_http(&self, target: &str, handler: &Fetcher) -> Result<()>;
>     pub async fn intercept_all_outbound_http(&self, handler: &Fetcher) -> Result<()>;
>     pub async fn intercept_outbound_https(&self, target: &str, handler: &Fetcher) -> Result<()>;
> }
> ```

## Gotchas

1. `Function::call1` on a loopback factory serializes the stub and throws; use `Reflect::apply`.
2. workers-rs 0.9 needs a recent workerd (`getActiveSpan`); pin wrangler 4.16x or later.
3. `wrangler dev` containers fail on native Linux Docker (workers-sdk #15394). Local tests of the
   container path need a VM, a different host, or hosted preview; CI on GitHub's Ubuntu runners
   is also unverified.
4. `ContainerStartupOptions` in 0.9: `From<..> for Object` deletes properties when they are set
   (inverted check; harmless only because the fields are prototype getters) and sends `env` as a
   `Map`. Build the options object with `Reflect` if env vars matter.
5. Container stdout has no stream in 0.9 except through `exec` (unbound). The entrypoint's own
   stdout goes to logs only. Either the builder serves its events on a port (the `/run` shape) or
   the DO `exec`s it.
6. DO `fetch` takes `worker::Request`, not `http` types (#582), so DO routes parse URLs by hand.
7. `Date::now()` inside workerd does not advance during compute; time only spans across awaits.
8. The `/tmp` user quota on this machine filled during the spike; `cargo install worker-build`
   needed `TMPDIR` on the home volume. The spike's `target/` is 473 MB.
