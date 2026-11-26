# Decisions

Each decision has a number that never changes, a status, what was decided, why, and what would
reverse it. "Owner" marks a decision the owner made; the rest were proposed in `research/` and
accepted with the spec on 2026-11-26. A superseded decision stays, with a pointer to its
replacement.

## 0001 Own engine, written from scratch

Accepted, owner. uses.ink gets a new engine; baudelaire is not reused, though its ideas may be.
**Why:** the old code was built around git and a fixed theme; the new contracts start clean.
**Reversal:** none planned.

## 0002 Nothing hardcoded: four plugin slots

Accepted, owner. Content comes through `source` plugins, ownership through `authority` plugins,
reading through `format` plugins, rendering through `theme` plugins. Git is one source, not the
protocol. First-party plugins use the public contract with no private API.
**Why:** the owner rejects hardcoding; one contract keeps first-party honest.
**Reversal:** none.

## 0003 Rust everywhere, one workspace, one binary

Accepted. Engine, control plane, edge, builder and CLI are Rust in one workspace; the binary is
`ink`. **Why:** the control plane would otherwise exist twice (Worker and self-host); shared
types without codegen. **Reversal:** if workers-rs blocks the edge, only the edge adapter moves
to a thin TS Worker over the Rust control plane compiled to wasm.

## 0004 Hosted runtime: Worker edge, Container builds

Accepted. A workers-rs Worker serves, runs the control plane and takes webhooks; every build runs
`ink build` in a Cloudflare Container started by a per-handle Durable Object. **Why:** a 128 MB
isolate cannot clone repos with history or render large vaults; one build path for every source.
**Reversal:** container cold starts above about 15 s push-to-live: add a wasm fast path for small
snapshots. Price or availability changes: the same binary runs on a VPS behind the same queue.

## 0005 Self-host is `ink serve`, one process

Accepted. SQLite, a blob directory, an in-process builder, optional ACME; zero flags required,
every default announced as a notice. **Why:** self-hosting must stay easy; D1 is SQLite, so
migrations are shared.

## 0006 The engine is pure

Accepted. The engine has no I/O; it asks host traits for everything, and every read is recorded
and becomes part of a render unit's cache key. **Why:** determinism, incremental builds without
declared dependencies, the same engine natively and in wasm.

## 0007 WIT is the plugin boundary; builder-side plugins are components in v1

Accepted. Plugin boundary types are written in WIT (`ink:*@0.1`). Source, format and theme
plugins are wasm components run by wasmtime in the builder from v1. Authority plugins are native
crates against the WIT-generated types, because the edge is a Worker and cannot host wasmtime.
**Why:** follows from 0015; once the host exists, every builder plugin goes through it, which
proves there is no back door. **Reversal:** if component overhead on formats is measured above
20% of build time, format plugins become native crates against the same types until it is fixed.

## 0008 `git.fetch` is a host capability

Accepted. Git runs host-side with gix; the git source plugin maps it to a snapshot. **Why:** it is
the one operation that needs a real disk and a native library.

## 0009 Content addressing with BLAKE3, scoped per tenant

Accepted. Every blob is keyed by its BLAKE3 hash under its tenant's prefix; no cross-tenant
dedup. **Why:** exact deletion, no existence leak through dedup timing.

## 0010 Static builds; a deploy is a hashed manifest

Accepted. Nothing renders on the reader path. Publish and rollback are pointer writes. **Why:**
atomic publish, trivial rollback, edge-cacheable reads. **Reversal:** per-reader content
(members, private posts behind auth), not in scope.

## 0011 Facts carry provenance and completeness

Accepted. Dates and authors are `Known<T>`: exact, a bound, or unknown with a reason. **Why:** a
shallow clone knows a bound, not a date; treating one as the other is a silent lie.

## 0012 Abilities are declared, then observed

Accepted, owner on the word. Sources declare `snapshot`, `history`, `watch`, `private`; each
source instance records what it actually had. Every lack maps to a stated consequence. "Ability"
is what a source can do; "capability" is a host function a plugin may import. **Why:** a missing
ability is shown, never guessed; one word, one meaning.

## 0013 Authorities are separate from sources

Accepted. Proofs are typed and dated; a handle's right to a source is a proof, not a URL. A forge
package may provide both an authority and a source.

## 0014 Theme contract: the engine owns structure, themes render pages

Accepted, owner. The engine owns routes, feeds, sitemap, images, CSP and headers. A theme
exports `describe`, `assets`, `render` per page, and may declare extra routes with `routes` and
`render-route`; collisions are diagnostics. **Why:** every theme gets correct feeds, links and
security headers, caching stays per page, and themes still have room for unusual pages.
**Reversal:** a real theme need that cannot be met by extra routes.

## 0015 Themes are wasm components in v1

Accepted, owner. Themes are WebAssembly components from the first release, written in any
language that targets components; first-party themes are Rust. Template overrides are something
a theme offers through the `asset.override` capability. **Why:** third-party themes on day one,
sandboxed, with the same contract first-party themes use.

## 0016 A quiet default theme; the hand-drawn theme is opt-in

Accepted, owner. `ink:theme-quiet` is the default: calm, the author's voice. `ink:theme-notebook`
carries the uses.ink look and is chosen in `ink.toml`. uses.ink signs only the "made with"
footer. **Why:** a reader site is the author's blog, not ours.

## 0017 Private by default for vaults and private sources

Accepted. Vaults and private sources publish nothing until the author selects; an attachment no
published page uses is never published. Public repos publish what Interpret classifies as pages.

## 0018 One config file, `ink.toml`; the decision code is the pin path

Accepted. One optional file at the source root. Every key pins a decision or chooses a slot
plugin; the key path is the decision code. Unknown keys are diagnostics. The dashboard holds no
interpretation settings. **Why:** one source of truth, in the author's source.

## 0019 Custom domains through Cloudflare for SaaS, bound by our TXT proof

Accepted. CNAME to `domains.uses.ink` after a TXT proof binds the name to a handle. Apex only
with CNAME flattening; otherwise `www` plus a redirect. **Reversal:** buy apex proxying if apex
requests dominate support.

## 0020 Isolation: PSL, `app.uses.ink`, no JS on reader pages

Accepted. `uses.ink` goes on the Public Suffix List before launch; the dashboard is its own site;
reader pages have no JavaScript, no cookies and a strict CSP written by the engine.

## 0021 Budgets are typed refusals, published

Accepted. Every limit refuses with the number and the limit, and all limits are on a public
page.

## 0022 Deletion removes bytes before rows

Accepted. Unpublish, delete blobs under the tenant, delete rows, send a receipt. A crash leaves a
diagnosable row, never orphaned bytes.

## 0023 The build log is one typed event stream

Accepted. The same events in the terminal, the dashboard, storage and forge statuses.

## 0024 Names

Accepted, owner. Product and site: uses.ink. Binary: `ink`. Config: `ink.toml`. Crates:
`uses-ink-*` (never `ink-*`, which collides with Polkadot's `ink_*` on crates.io). WIT and plugin
scope: `ink:`. Conventions in `conventions.md`.

## 0025 One pen

Accepted, owner. Every hand-drawn mark in the product comes from one Rust crate: themes through
the `pen` capability, the dashboard and landing through its wasm build, the logo through `cargo
xtask brand`. No JS copy, no generator scripts. **Why:** the owner wants shared machinery
centralized; one implementation cannot drift.

## 0026 Licence

Accepted, owner. Service, engine and first-party plugins: AGPL-3.0-or-later. SDK, WIT packages,
pen and other product-independent crates: MIT OR Apache-2.0.

## 0027 Markdown through comrak; raw HTML through our own allowlist

Accepted. comrak is the only parser with nodes for wikilinks, embeds, callouts and block ids,
with spans. Raw HTML is parsed into `Document` against our allowlist; ammonia only behind an
escape hatch. **Why:** pulldown-cmark had two 2026 wikilink advisories; ammonia had four mXSS
advisories in 2026 (`research/deps-engine.md`).

The allowlist, as the markdown format reads it: `<kbd>`, `<sup>`, `<sub>`, `<mark>`, `<del>` and
`<s>`, `<br>`, `<img src alt>` and `<a href>` inline; `<details>` with an optional first
`<summary>` around markdown, `<hr>` and `<br>` as blocks. No attribute is kept beyond `src`, `alt`
and `href`, which follow the same rules as markdown images and links. Every other element is
removed with `html.element.stripped` and its text kept.

## 0028 Dates with jiff

Accepted. jiff 1.0 with our own fixed locale table; chrono is unmaintained (RustSec 2026).

## 0029 Repo tasks are `cargo xtask`

Accepted. Brand generation, WIT codegen checks, fixture updates and the convention checks are
xtask subcommands. **Why:** no loose scripts (0025's reason applies to every tool).

## 0030 The `Db` port is shaped like D1

Accepted. Queries and atomic batches of raw SQL, no interactive transactions, one migration set
in wrangler's format with our own runner for SQLite. **Why:** D1 is the stricter target; a port
shaped like SQLite would let self-host code do what hosted cannot. **Reversal:** D1 gains
interactive transactions.

## 0031 Credentials sealed with XChaCha20-Poly1305

Accepted. Sealed per row with the row id as associated data, so a copied row fails to open. This
replaces the AES-256-GCM suggestion in `research/architecture.md`. **Why:** a 24-byte random nonce
needs no counter; `age` and `hpke` cannot share a build today.

## 0032 HTML, SVG and CSS through `unspliced`; no maud, no Datastar

Accepted, owner. Every page uses.ink renders (themes, dashboard, landing) is written with
`unspliced`, a crate of its own (`~/code/personal/unspliced`, MIT OR Apache-2.0). It rebuilds the
owner's earlier `press` (stik) and `vun-html` (vun) as one library: names are typed constants,
documents are baked by the compiler, text escapes itself, URLs from run time are checked, and
there is no JavaScript runtime: behaviour is native HTML (`popover`, `<details>`, `<dialog>` with
`command`), forms that post and redirect, cross-document view transitions, and streamed pages
(`Split`) for live build logs. A typed action runtime was built and dropped on 2026-11-26 as a
private framework the platform no longer needs. **Why:** maud lets typos compile and leaves CSS and JS untyped;
Datastar puts untyped expressions in attributes and needs `unsafe-eval` or a nonce. **Reversal:**
none planned; Leptos remains the documented fallback for the dashboard.

## 0033 The pen crate is `freehand`; brand drawings are data

Accepted, owner. The pen of 0025 is the crate `freehand` at `freehand/` (MIT OR Apache-2.0, `no_std`,
only `libm`). The logo, mark and favicon are described in `brand/figure.toml` (named polygons,
shapes, weights, colours by token name) and drawn by `cargo xtask brand`; the SVG files are written
with `unspliced`, whose bare `xmlns` on `svg` writes the namespace a standalone file needs.
**Why:** one implementation of the pen; geometry is data, so a drawing changes without code.
**Reversal:** none planned.

## 0034 The pen in the browser is plain wasm and one small loader

Accepted. `freehand-web` exports the marks (`line`, `underline`, `frame`, `fill`, `patch`,
`drawn`) as plain wasm functions: numbers and a name in, SVG path data out, through two buffers
the module owns. `freehand-web/freehand.js` (about 40 lines) loads it; the page measures elements
and inserts the paths. The mock's `pen.js` now draws through it, and `pen-core.js`, `gen-*.mjs`
and the vendored perfect-freehand are gone. The workspace denies `unsafe_code` instead of
forbidding it, because a wasm export's name needs `#[unsafe(no_mangle)]`; `xtask check` allows
unsafe code in `freehand-web/src/lib.rs` and nowhere else. **Why:** `wasm-bindgen` generates JS
glue on every build, against 0032; marks pass a few numbers, so the protocol is small enough to
own. **Reversal:** if the dashboard needs many more exports, revisit `wasm-bindgen`.

## 0035 Plugins run on wasmtime as written; guests target `wasm32-unknown-unknown`

Accepted. The M0 spike (`spikes/wasmtime/README.md`) ran a Rust theme component importing
`pen.stroke` (the real `freehand`) and `text.measure` on wasmtime 51: a pooled instance costs
about 6 us, a guest-to-host import about 150 ns, a `.cwasm` load 58 us against a 12 to 50 ms
Cranelift compile, and fuel with epochs about 7% of a render. The pen, not the boundary, is the
cost of a page. Host types come from `wasmtime::component::bindgen!`; guests use `wit-bindgen`
0.63 built for `wasm32-unknown-unknown` and wrapped by `wit-component`, because a `wasm32-wasip2`
build imports 14 WASI interfaces (clocks, environment, stdin) that plugins may not have.
`cargo-component` is not used. Follow-ups: `host/wasmtime` carries one reasoned `unsafe` for
`Component::deserialize`, so `xtask check`'s allowlist (0034) gains that file when `host/wasmtime` lands; the `.cwasm` cache is
keyed by plugin digest plus `Engine::precompile_compatibility_hash`; authorities need a plain
types crate, since the generated types derive wasmtime traits. **Why:** the measured overhead is
small next to the work a page does. **Reversal:** a real theme where instantiation or imports
exceed 20% of render time.

## 0036 Git history through gix: full clone for history, one walk for dates

Accepted, spike (`spikes/gix/`). The `git` capability uses `gix` 0.91. When history is wanted it
clones in full; depth 1 plus `deepen` is for a snapshot first or a bounded window, never as the
road to full history. File dates come from one walk over history with a parallel tree diff per
commit (merges skipped, renames not followed), and files older than a shallow window get
`NotAfter`. **Why:** on tldr (40,944 files, 24,903 commits) the dates take about 2 s on 4
threads in 300 MiB; a direct full clone takes 7 s against 17 s for depth 1 plus a full deepen.
The 5 s budget holds for the dates, not for the fetch. gix has no blobless partial clone, so the
`filter=blob:none` idea in `research/architecture.md` is dropped. **Reversal:** gix gains
partial clone, or rename following turns out to matter for real sources (1.8% of tldr's files
were created by a pure move).

## 0037 Container egress from Rust through `Reflect`; no JS shim

Accepted, spike (`spikes/workers/`). The build Durable Object starts the Container with
`enableInternet: false` and registers outbound intercepts itself: `ctx.exports.default({ props })`
(called with `Reflect::apply`) is the handler, so the worker's own `#[event(fetch)]` is the egress
policy and tells intercepted requests apart by their props. `exec` and the intercepts are called
through `js_sys::Reflect` until workers-rs binds them (#1072; PRs #1078, #1143, #1144). The
handler, not the container, attaches the forge credential. Live logs reach the DO over the
builder's HTTP port or `exec` stdout; the DO streams them and writes the log to R2. **Why:**
DO streaming, R2, the loopback handler and intercept registration all ran from Rust locally, so a
shim and an internet-on v1 are not needed. **Unverified:** the container never started under
`wrangler dev` on native Linux Docker (workers-sdk #15394), so start time, the container-to-DO
stream and enforced egress wait for a host where local containers run, or a hosted preview.
**Reversal:** hosted runs show intercepts do not accept the default-entrypoint loopback.

## 0038 Path data in tenths, as a `T` chain; lengths by `sqrt`

Accepted, owner. Paths are written in whole tenths of a pixel (no leading zero, no trailing
`.0`), as one `Q` and then a `T` chain: mirroring a control through the midpoint to the next
gives exactly the next, so the controls after the first are implied. Each anchor is rounded so
the next implied control lands on its true point, which keeps every control and anchor within a
tenth of a pixel per axis with no drift. Lengths are `sqrt(x² + y²)` instead of `hypot`. Output
is no longer byte for byte the mock's JS (0033's parity claim ends); native and wasm still agree,
and the golden fixtures are the reference. **Why:** a 336 px line went from 96 us to 10 us (with
the formatting work of the same day) and brand files shrank 58%, with no visible change at 4x;
the ink is drawn without anti-aliasing, so hundredths never showed. **Reversal:** a theme drawn
large enough that a tenth shows.

## 0039 Highlighting is a host capability, through arborium

Accepted, owner. Code is highlighted by the host, behind a WIT interface `ink:host/highlight`
next to `pen` and `text`: a theme asks for a code block's spans by language and gets classed
spans back, then styles the classes. The host implements it with arborium (tree-sitter grammars,
only the languages we enable as features). A theme never links a highlighter. **Why:** every
theme highlights the same way, as with the pen; tree-sitter parses more accurately than regex
grammars; C grammars stay in the native host, so themes stay small pure-Rust components; and
giallo's prebuilt bundle needed `zstd-sys` (C) inside the theme and loaded every grammar up front
(about 40 ms). **Fallback:** giallo without its `dump` feature, with grammars bundled by an xtask
generator, if arborium's C toolchain gets in the way. **Reversal:** a theme needs a highlighting
style tree-sitter's capture names cannot express.

As built (2026-11-26), natively until `ink:host/highlight` exists in WIT: the trait is
`model::highlight::Highlight`, in model and not the engine, because the engine never highlights
and the theme and host-native both reach model already. It returns `Run`s (an optional `Capture`
and the text they cover) or a typed `Unhighlighted` (`Unknown`, or `Failed` with the host's
error), so `code.language.unknown` comes from a variant, never a string. `Capture` is a closed
set of 27; host-native maps tree-sitter capture names onto it through `arborium-theme`'s slot
table, in one exhaustive `match`, and flattens nested and crossing captures so the innermost
wins. arborium 2.18 with 14 grammars (bash, c, css, diff, go, html, javascript, json, markdown,
python, rust, toml, typescript, yaml) builds its C in about 12 s on a first release build and
adds 4.7 MB to `ink` (13.3 MB to 18.0 MB). A grammar compiles on first use, once per process:
31 ms for Rust, 36 ms for TypeScript, 3 to 5 ms for Markdown or Python in release; 218 lines of
Rust then highlight in 2 ms. Its licences are MIT OR Apache-2.0, so `deny.toml` needs no
exception, and giallo, zstd and their exceptions are gone. host-native builds for
`wasm32-unknown-unknown` with the system clang (arborium ships its own sysroot), so the plan
holds if the host ever runs in wasm; the theme no longer links any highlighter, nor C.
