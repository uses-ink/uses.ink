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

Accepted. The M0 spike (`spikes/wasmtime/`, removed once `host/wasmtime` replaced it; its README
is in the history) ran a Rust theme component importing
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

As built (2026-11-27), natively, before `git.fetch` and `ink:source-git` exist as components:
`host_native::Repository` reads a local repository at one commit (`HEAD`, or `--rev` as
`git rev-parse` takes it) into a `Snapshot` whose version is `Version::GitCommit`. The snapshot
is the commit's tree, never the working copy, so `ink build` in a repository builds what is
committed; it says so with `source.git.chosen`, and `--source folder` reads the files on disk
instead. The tree is read with the folder's budgets and refusals: links are resolved against
the tree (a link to a file is that file, to a folder is skipped, out of the tree is refused,
to nothing is `source.link.broken`), submodules are skipped and LFS pointers kept and said. A
file's `updated` is the newest commit by author time that touched its path and its `created`
the newest that added it, both `Known::Exact` from `Provenance::Commit`, so a file deleted and
added again was created when it came back. A shallow boundary gives the rest `NotAfter` from
`Provenance::GitHistory`; a change seen only in a merge is `Unknown { Merged }`. What the
history lacked is a `Lack` with a `Gap` (`Shallow`, or `Renames` with the count of files a
move created), said as a notice and shown by `ink explain`. gix is opened isolated (no user or
system configuration) with an 8 MiB delta-base cache per thread (as measured, below) and
without `pack-cache-lru-static`, whose `uluru` is MPL-2.0; `foldhash` and `zlib-rs` (Zlib) are
licence exceptions in `deny.toml`. Measured on tldr (40,943 files, 24,903 commits, full clone,
release build): the whole read, every blob and every date, takes 10.1 s on 1 thread, 3.3 s on 4
and 2.6 s on 8, in 150 MiB; it matches the spike's 732 files created by a move.

As measured (2026-11-27, performance pass, `cargo bench -p uses-ink-host-native` with
`INK_BENCH_REPO` naming tldr; the machine shared with other builds, so CPU counts are the firmer
numbers): the `core.deltaBaseCacheLimit=32m` override above never took effect. gix sets a
delta-base cache from it only under its `max-performance` feature, which pulls the MPL code, so
every tree was inflated from its pack again, and zlib was 68% of the profile. The cache is now set
on each thread's handle in code, 8 MiB each: the read with dates went from 4.6 s to 2.7 s on 4
threads and from about 170 to about 85 billion instructions, at 224 MiB peak instead of 156 MiB.
4, 8, 16 and 32 MiB all land at 2.7 s; 64 MiB is slower. An object cache on top of it made the
diffs slower, not faster. Of the 2.7 s, the tree read (every blob inflated and hashed) is 0.35 s
and the commit walk 0.2 s; the rest is the parallel tree diff, still mostly inflate. Left alone:
the diff algorithm (gix's tree diff is a merge of sorted entries, already linear), object lookups
(the pack index lookup is under 1%), and allocation (`memmove` is 8%, mostly delta application).
The folder walk reads a generated tree of 20,000 one-kilobyte notes in about 160 ms, almost all
of it system calls and BLAKE3; it stays on one thread, which keeps its order trivially fixed.

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

As measured (2026-11-27, `research/performance.md`): highlighting is 79% of rendering a post
with eight code blocks, nearly all of it tree-sitter's parse and query; a 25-line Rust block
takes 2.7 million instructions, about 270 us. Reusing the parser between calls saves 0.5%, so
`Grammars` still makes one per call.

## 0040 Images are prepared by the host, deterministically, and served as hashed copies

Accepted. The engine decides which files get copies, their routes, headers and what a theme
learns of them; the host does the pixel work behind `engine::Images`, as with highlighting
(0039), in `host/image` (`uses-ink-host-image`). Before any page is rendered, each raster asset
is decoded within a pixel budget (50 million by default; a header over it is
`image.budget.pixels`, refused before a pixel is decoded), its EXIF orientation applied, resized
with Catmull-Rom to the widths 480, 960 and 1440 that are narrower than it (plus its own width;
never wider), and encoded as AVIF (ravif, quality 70, speed 9) and JPEG (quality 80,
baseline with optimized tables), or PNG when it has transparency. Re-encoding leaves every bit of metadata behind,
so the original is not served. Each copy is served at `<name>.<width>.<hash>.<ext>` with
`cache-control: public, max-age=31536000, immutable`; the asset carries a `model::Picture` (its
size, its copies, its placeholder) for the theme's `width`, `height`, `srcset` and `<picture>`.
The placeholder is the image's thumbhash drawn as a 32-pixel PNG, served as a hashed file and
shown under the image, not inlined: `unspliced` refuses a `data:` URL at run time and
`style-src 'self'` refuses inline styles, so the CSP stays as it is. Animated GIF and WebP are
served as they are (no reader page runs script, so a poster could not start them), and AVIF is,
since decoding it takes dav1d (C); each is a notice.

Determinism, measured: ravif cuts as many tiles as rayon has threads, so its bytes followed the
core count; the work runs rav1e on one thread (one tile) without assembly, jpeg-encoder without
its SIMD feature, `fast_image_resize`'s SIMD paths give the scalar path's bytes for this filter,
and the ported thumbhash takes its cosines from `libm`. The goldens in `host/image/fixture` hold
the hash of every copy. A `Key` (the source's hash, the settings, the crates' versions) names
one image's work; `host_image::Pixels` keeps what it made by key, and keeps it across builds
in an `engine::Cache` (as measured, below). **Why:** layout stability and small pages for readers,
no metadata leaks, no C. **Reversal:** encode time on hosted builders (a 4000 by 3000 photo
costs about 5 s of one core at speed 9) pushes builds past their budget: speed 10 again, or
fewer widths.

As measured (2026-11-27, performance pass; ten 4000 by 3000 photos cut from one wallpaper, 8
workers, release build, on a 14-thread laptop shared with other builds):

| Build | Wall | CPU | Peak memory |
|---|---|---|---|
| 10 photos, before | 7.1 s | 39 s | 733 MB |
| 10 photos, cold cache | 5.1 s | 37 s | 564 MB |
| 10 photos, warm cache | 0.37 s | 0.2 s | 45 MB |
| 1 photo, before | 2.6 s | 2.5 s | 111 MB |
| 1 photo, cold cache | 1.7 s | 2.4 s | 122 MB |

Every build gives the same deploy, byte for byte. Three changes. **The cache:** an
`engine::Cache` port (typed `cache::Key` of a `Kind` and a digest, values as bytes), the first
piece of the render unit index; `host_native::Disk` keeps it in a folder, `--cache <dir>` or
`<site>/.ink/cache` by default, so `rm -r .ink` resets everything ink owns and a hosted
builder's cache stays per tenant. Values are files named by their BLAKE3 hash, each key an
entry listing its values and ending with the hash of that list; every file is written under a
name of its own and renamed into place. A value or entry that does not check out is removed and
said (`cache.entry.corrupt`) and the image made again; a folder made for the first time says
`render.cache.cold`; past 2 GiB the entries used longest ago go (`cache.entry.evicted`). The
folder holds `.ink-cache`, so the folder walk and the git tree skip it wherever it is
(`source.cache.skipped`). **The work:** an image is decoded on one thread, cut into its widths,
and its full-size pixels dropped; every copy is then a job, the costliest first across all
images, so one photo uses several cores and ten do not wait on the last one's widest AVIF. The
resizer no longer copies the full-size pixels for each width. **The memory:** at most two images
are decoded at once; what is left is mostly eight rav1e encoders at once (a 1440-wide encode
holds some 40 MB), and one decode slot instead of two saved about 3%. The bytes do not follow the
number of threads: a test prepares every fixture on 1, 2, 3 and 8 and compares.

The CLI site tests share one cache under `target/`, so after the first run the blog's photos
are taken from it; their photos (64 by 32 and 48 by 32) were already small enough to encode
within budget cold, so the `HACK(performance pass)` override in `.config/nextest.toml` is gone
and the blog tests run under the highlighting budget they carry anyway. The override also hid
that the test writing pins back built all five sites in one process, 300 ms beside the rest of
the suite even with every photo cached; it is one test per site now.

Not changed, since each changes what readers get; each measured on one photo, CPU time on one
core, SSIM against the resized original:

| Change | Cost | Bytes | SSIM |
|---|---|---|---|
| AVIF speed 9 instead of 10 (taken, below) | +40% to +55% | -13% to -15% | 0.932 to 0.935 at 1440, 0.924 to 0.942 at 480 |
| AVIF speed 8 or 6 | x2.6 to x3.9 | -15% to -19% | at most 0.004 over speed 9 |
| AVIF 8-bit instead of 10-bit (ravif's default) | 0% to -19% | +5% to +7% | 0.006 to 0.010 lower |
| JPEG baseline with optimized Huffman tables (taken, below) | same | -16% at 1440, -13% at 480 | identical pixels in libjpeg |
| JPEG progressive with optimized tables | same | -6% to -8% | identical pixels |
| No 1440 copy | -60% of AVIF time | | 3x screens get 960 |

The 1440 AVIF is 1.4 to 1.7 s of a photo's 2.5 s; the 960 is 0.66 s, the 480 0.22 s, every JPEG
together under 0.1 s, the decode 65 ms and the three resizes 130 ms. Speed 9 behind the cache and
optimized Huffman tables are the two worth taking; jpeg-encoder's progressive mode makes files
larger, not smaller, and the placeholder already covers what progressive loading would show.

Speed 9 is taken (2026-11-27), behind the cache. Measured on the same machine, 8 workers,
release build, but on generated photos (ten seeded `magick plasma:fractal` images, 4000 by 3000,
with noise), since the wallpaper is not in the repo:

| Build | Wall | CPU | Peak memory | AVIF bytes, 30 copies |
|---|---|---|---|---|
| 10 photos, speed 10, cold cache | 5.1 to 5.6 s | 39 to 42 s | 586 MB | 1,022,036 |
| 10 photos, speed 9, cold cache | 8.7 to 8.9 s | 65 to 67 s | 644 MB | 871,687 (-14.7%) |
| 10 photos, either, warm cache | 0.05 s | 0.05 s | 60 MB | |
| 1 photo, speed 10, cold cache | 1.8 to 2.1 s | 2.7 to 3.0 s | 124 MB | |
| 1 photo, speed 9, cold cache | 3.1 to 3.4 s | 4.6 to 5.2 s | 133 MB | |

A cold build costs about 65% more CPU on these photos, more than the one-core estimate in the
table above; a warm build costs the same. The JPEG copies are unchanged.

Baseline JPEG with optimized Huffman tables is taken, with its own decoder. jpeg-encoder 0.7.1
writes optimized tables only in its sequential mode, one scan per component, and zune-jpeg 0.5.15
(the decoder `image` 0.25 pins) misread such a file with 4:2:0 chroma: against the source,
libjpeg's decode of the 48 by 36 fixture was 31 dB and zune-jpeg's 5 dB, and a 1600 by 1200
photo's copies came out at 8 dB beside libjpeg's at every width. The file is valid and browsers
decode it with libjpeg-turbo. zune-jpeg 0.5.16-rc3 reads it (a 96 by 72 probe: 5.7 dB on 0.5.15,
37.7 dB on rc3, the same as the other three sampling and table modes), so `host/image` depends on
`zune-jpeg =0.5.16-rc3` directly, turns off `image`'s `jpeg` feature, and reads JPEG itself
(`host/image/src/jpeg.rs`: size from the headers, EXIF orientation, grey or RGB pixels, CMYK
converted). A prerelease cannot satisfy `image`'s `^0.5` requirement, so no `[patch]` can swap it
in under `image`; that is why the reader is ours. The cache key's codec tag is no longer typed by
hand: `host/image/build.rs` reads the versions of the crate's registry dependencies from
`Cargo.lock`. The fixtures' JPEG copies shrink (773 to 310 bytes, 1074 to 599) since baseline
carries less overhead than the progressive files they replace. **Reversal:** zune-jpeg 0.5.16
is released and `image` accepts it: drop the direct reader and turn the feature back on.

## 0041 Components as built: values across, one shared mirror

Accepted. `ink:format-markdown` and `ink:theme-quiet` run as components through `host/wasmtime`,
and `ink build --check` builds every CLI site fixture the same, byte for byte, natively and
through them. What 0035 and architecture.md sections 6 and 7 did not settle, as built:

- **The WIT lives at `wit/<package>/`**: `ink:model` (the model's types), `ink:host`
  (capabilities, and a `plugin` world every binding is generated from once), `ink:format`,
  `ink:theme`. Each slot's world maps its imports onto the `plugin` bindings with `with`, so one
  set of types exists per side. Generators get several paths in dependency order; no `deps/`
  copies.
- **The model crosses as values.** A document is flattened into two lists (blocks, inlines) whose
  nodes name their children by place, children first, each used once, and is rebuilt in one
  pass without recursion, so hostile input cannot make a cycle, share a node or overflow the
  host's stack. A problem or announcement crosses as its code and its fields in catalog order
  (`model::coded`), so a new catalog entry changes no WIT. A diagnostic's cause chain is a list.
  Each asset carries the `picture` the engine prepared (0040: size, copies, placeholder), so a
  theme component writes `<picture>` exactly as the native one does.
- **A theme renders one site per instance.** Its `pages` export is a `renderer` resource built
  from the base address and the site; each page is then one call. The adapter keeps the session
  while the engine passes an equal site and starts a new instance otherwise or after a failed
  call. A format call gets an instance of its own (about 6 us, 0035). Architecture's host-owned
  `site` resource with lazy reads is not built: the native core takes a whole `Site`.
- **Conversions are written once for both sides.** `wit/mirror/` holds them as `From` and
  `TryFrom` impls naming the generated types through `super::wit`; `host/wasmtime` and
  `plugin/sdk` each include its parts over their own bindings, since `bindgen!` and
  `wit-bindgen` write the same shapes but no common type.
- **Memory is limited by a `ResourceLimiter`**, not the pooling allocator, so a refused growth is
  told apart from other traps (`plugin.budget.memory`). Fuel and an epoch ticker (5 ms) give
  `plugin.budget.fuel` and `plugin.budget.time`; a trap is `plugin.call.trapped`, a value the
  model refuses `plugin.value.malformed`. Every one is a diagnostic and the next call is served.
- **`text.measure` refuses every font** (`font-unknown`): the measurer is not built.
- **Components are read from a folder** (`--components`, written by `cargo xtask component` to
  `target/plugin/`), not bundled in `ink`; their `.cwasm` cache is its `cache/` folder, each file
  checked by its BLAKE3 before it is loaded, a file that does not check compiled again.
- **The model, the SDK and `wit/` are MIT OR Apache-2.0** (0026). The mirror converts to
  nearly all of the model: a site names decision codes, whose evidence holds a snapshot's
  provenance, and an asset's picture holds a manifest's media type, so a cut of the types that
  cross would have left `model` an empty shell around `Snapshot` and `Manifest`. The model is
  plain values with only permissive dependencies, so it is relicensed whole instead of cut.
  `Record`, `Said` and a theme's `File` (the WIT's `ink:model/file`) moved from the engine to
  the model, which the engine re-exports, so the SDK depends on `freehand` and `model` only. `cargo xtask layout` refuses a permissive crate
  with a normal or build dependency on an AGPL crate of ours (`Licence`, `Problem::Copyleft`).
  The engine's plugin traits and their types (`Format`, `Theme`, `Html`, `BaseUrl`) stay AGPL: a
  component needs only the SDK, and a third-party native core written against them is a later
  decision.

Measured (release, this machine, `host/wasmtime/tests/measure.rs`): the markdown component is
1,369,744 bytes (464 KB gzipped, a 4.5 MB `.cwasm`), the quiet theme 657,499 (227 KB, 2.2 MB);
a Cranelift compile takes 402 ms and 244 ms on all cores, a cached load (a new engine, the digest
checked, `deserialize`) 10 ms and 5 ms. The blog fixture, interpreted and rendered, takes 0.31 ms
natively and 2.8 ms through the components: on five files the boundary is all of the cost, and
0007's reversal (formats over 20% of build time) is to be judged on a real site, not on this.
`ink build --check` of that fixture takes 0.77 s on a cold cache and 66 ms warm, in a debug build.

**Why:** the native cores stay as they are, the boundary is written once, and the check builds
the same bytes both ways. **Reversal:** a site large enough that sending it to each theme
instance shows in build time; then `site` becomes a host resource read lazily.

## 0042 Decisions carry one evidence; changes carry a cause; inference rules are versioned

Accepted, owner (`research/inference.md`). A decision is its value and the evidence that chose
it; rejected alternatives leave the model, and `ink explain --why <path> <code>` replays one
ladder on request, rung by rung, including the rungs that found nothing. Every build is compared
with a baseline (the live deploy hosted; locally the previous `ink build`, whose output folder
keeps a decision record beside the manifest so causes can be told apart) and reports a change
set in which each change has a typed cause: content, config, rules, plugin, ability or engine.
Newly public routes head every change set, and a private source refuses any publication that no
stated selection made. Inference rules have a version, decided as `site.rules`: a pin, else the
baseline's, else the latest. A newer ink never changes a site's decisions on its own: what its
rules would change is shown as pending until `ink update` or a dashboard action accepts it.
`ink explain --pin` is removed; `ink pin <path> <code>` writes one pin. The owner's answers:
a deploy may hold the rules version when the source states none, labelled as held outside the
source (E1, refines 0018); old rules are kept for two rule versions or twelve months, whichever
is longer (E2); bundled theme updates go live without acceptance and are listed in the change
set (E3); the first hosted deploy suggests committing `rules = n` as a notice and never writes
it (E4); a change attributed to the engine is a warning locally and holds a hosted deploy (E5).
Built in two steps: one evidence, `--why`, the change set, the newly public guard and `ink pin`
in M2; versioned rules and amendments before hosting, in M4. **Why:** the jobs that cannot be
undone (a page made public, a site moved by our upgrade) are questions about change between
builds, which per-value evidence cannot answer; locking rules instead of values keeps content
free and needs no commit, so vaults and uploads are covered; the baseline is state already kept
for rollback. **Reversal:** amendments are needed more than about once a quarter, or replaying
interpret under two rules costs more than 10% of build time on the largest fixture: drop pending
replays and keep a published rules changelog with the change set alone. Refines 0011 (bounds
stay facts), 0018 and the M1 exit criteria ("pin reproduces the deploy" is dropped: it tested
that pins freeze content).

As built (M2, step 1): `Decision<C>` is subject, value and evidence; `Evidence::basis()` says
`Stated`, `Inferred` or `Assumed`. `ink explain --why [path] <code>` replays one ladder through
`Engine::trace` (`uses.trace/v1`); `uses.explain/v2` drops `rejected`. `ink pin [path] <code>`
writes one pin (`uses.pin/v2`, `--dry-run`). `ink build` writes `uses.manifest/v3`, whose
`record` holds the build's inputs, decisions, files and routes, and prints the change set
against the previous record on stderr (in `uses.build/v2` with `--json`); a record it cannot read
is a notice, never a refusal. The cause order adds two steps to `research/inference.md` 7.3: a
changed `ink.toml` is `config`, and a change to any other file is `content` before `engine`, since
a file's decision can follow another file (an image a page stops using); and `ability` compares
what the source lacked as well as what it had, so a deepened shallow clone is the source's change,
not ink's. The guard is
`page.publish.unstated`, as diagnostic codes have three words. Bundled plugins are not listed in
the record: they move with ink's release, so `plugin` is given only for components. `rules` is
typed and waits for `site.rules`.

`site.format` (0043) is the first code decided before the engine runs: it chooses the format the
engine is built over, so `engine::Chosen` decides it from the snapshot and the pins and
`Understood::with` adds it. `Evidence::Inline { path, span }` is the evidence of a value found
in a page's text; it is `Basis::Inferred`.

## 0043 Obsidian is a dialect of the markdown format; the engine resolves and embeds

Accepted. `ink:format-obsidian` (`plugin/format/obsidian`, its component in `guest/`) reads one
note through the markdown format's comrak setup and raw HTML allowlist (0027), as a dialect:
`format_markdown::dialect::Dialect` turns on the syntax comrak parses for it and says what its
own nodes mean, and `format_markdown::read_as` reads through the one builder, generic over the
dialect. Nothing in the dialect sees comrak. What it reads, as built:

- **Wikilinks** `[[Note]]`, `[[Note|text]]`, `[[Note#Heading]]`, `[[Note#^block]]`,
  `[[#Heading]]` become `LinkTarget::Unresolved` with the target as written (the last `#` part
  is the anchor), shown as Obsidian shows them (`Note > Heading`) when no `|` names the text.
- **Embeds**: `![[image.png]]` is an image whose alt text is the text after the `|` or the file
  name, a size there said (`embed.size.ignored`); `![[Note]]`, `![[Note#Heading]]` and
  `![[Note#^block]]` alone in a paragraph are the new `Block::Embed`; among other text they are
  the new `Inline::Embed`, which the engine turns into a `Block::Embed` between the paragraph's
  text before it and after it (blocks cannot sit inside text), and where only text fits (a
  heading, a table cell, a link's text) into a link, `embed.inline.linked`; other files (PDF,
  audio, video, canvas) are links (`embed.file.linked`).
- **Callouts**: comrak's Obsidian callouts, any kind (Obsidian's aliases mapped: `hint` is a tip,
  `tldr` an `abstract`), a title of inlines, and `Block::Callout::folding` (`Fixed`, `Open` for
  `+`, `Closed` for `-`), which the quiet theme writes as `<details>`. Metadata after `|` is said
  and dropped (`callout.metadata.ignored`).
- **Block ids**: `^id` ending a block, or alone on the line after one, is the new
  `Inline::Anchor`. Ids share a page's anchors with its headings; one whose slug is taken is
  `block.id.taken` and gets the next free one, so no link meant for the earlier anchor moves.
- **Comments** `%%x%%` and `%%` blocks are removed, as HTML comments are.
- **Tags** `#tag` and `#a/b` in the text (not in code or link text) are `Read::inline_tags`, each
  once by slug and with its own span, apart from the frontmatter's `Front::tags`. The engine
  joins them (below).
- **Aliases**: frontmatter `aliases` or `alias` fill the new `Front::aliases`.
- **Title**: a note's title is its file name, as Obsidian shows it, after a frontmatter `title`.
  The dialect says so (`Syntax::title`), and the format reports no first heading, so the
  existing title ladder (`Heading`, then the file name) finds none and falls to the name. The
  headings stay headings.

Obsidian lacks `^x^` superscript and definition lists, so they stay text. It also shows `~x~` as
written and strikes only `~~x~~`. comrak has no switch for that, so the dialect (`Extension::
DoubleStrike`) reads the source at each struck run and turns one that does not open with `~~`
back into the text it was; code spans and blocks never become a struck run, so they are
untouched.

The engine resolves, since a format reads one file. The link machinery is the existing one,
extended for every format: after the page-relative path, its extension and a folder's index, a
name is looked for anywhere under the root, letter for letter and then in any case, by file
name, stem or alias; a path with a `/` names the file whose path ends with it. This is
Obsidian's shortest-path resolution: a unique name or partial path resolves, several are
`link.target.ambiguous`, one unpublished is `link.target.unpublished`. The `Settings` stage reads
`attachmentFolderPath` from `.obsidian/app.json` under the root (`/`, `./`, `./name`, `name`;
`vault.settings.invalid` when it does not read); a name several files share settles on the one
in the attachment folder for the linking note, said as `link.target.attached`. Obsidian itself
never consults the setting to resolve; this is ours, and the notice keeps it from being a silent
guess. The same stage, run before `file.ignore`, reads the templates folder: `folder` in
`.obsidian/templates.json`, used while `.obsidian/core-plugins.json` (a list of the plugins that
are on, or a map of each to whether it is) has the templates plugin on or is absent; files under
it are never pages (`Folder<Templates>`, evidence naming the folder, in any case). With no
folder set, or the plugin off, the conventional `templates/` at the top stands, as before. Each of
the three files that does not read is `vault.settings.invalid` naming that file. The `Embeds` stage fills each embed of a published page with the page's blocks, the
section under the heading (to the next heading of its level or above), or the innermost block
holding the block id (the block before, for an id alone on its line). What is shown keeps
pointing at its own page: anchor links become links to it, its block ids are removed and its
footnote references dropped (`embed.footnote.dropped`). A page holds an `id` once, so a heading
it shows gets the next free `name-1`, `name-2` when the page, or an embed before it, already
has its anchor; the order is the document's, so the ids are the same on every build, and a link
inside the shown part still points at the embedded page's own anchor. An embed reaching a page and part it is
already inside is `embed.target.cycle`, one naming no part is `embed.anchor.missing`, past 64 per
page (embeds inside embeds counted) is `embed.budget.count`; each is shown as a link.

**Selection** follows the source's: `ink` reads a folder holding `.obsidian/` at its top as a
vault through `ink:format-obsidian`, said as `format.obsidian.chosen`, as a folder holding
`.git` is read as a repository (`source.git.chosen`). The vault layout (`Folder<Obsidian>`)
already publishes nothing until it is selected. The choice is the decision `site.format`, whose
value is a format plugin's id (`ink:format-obsidian`, `ink:format-markdown`) and whose ladder is
`Pin`, then `Folder<Obsidian>` (evidence `Marker { path: ".obsidian" }`), then the markdown
format. A pin is written `[site] format = "ink:format-obsidian"` and said as `format.pin.chosen`;
a vault found by its marker is `format.obsidian.chosen`; the default is silent. A pin wins over
the marker either way, so a vault can be read as plain markdown and a folder without
`.obsidian/` as a vault. An id naming a format this ink does not hold stops the command
(`format.plugin.unknown`). The format is read before anything else, so the host decides it
(`engine::Chosen`) and builds the engine over the format it names; the decision joins the
engine's with `Understood::with`, so `ink explain`, `--why` and `ink pin` treat it as any other,
and it is in the build's record. `ink build --check` builds a vault through
`format-obsidian.wasm`, and `cli/fixture/site/vault` proves both builds match.

**Tags** are the one decision both sources feed, since Obsidian shows a note's frontmatter tags
and its inline tags together. A ladder takes the first value, so `page.tags` is `Pin`, `Inline`,
`Front`, default. `Inline` (evidence `Evidence::Inline { path, span }`, the first tag only the
text said) testifies only when the text adds a tag the frontmatter lacks, and then its value is
the frontmatter's tags followed by the text's new ones, each slug once. When the text adds
nothing, `Front` decides as before, with the frontmatter as evidence. So a decision is one
value with one evidence, and the evidence is the least stated source the value depends on: the
frontmatter's alone, or the text where the text added to it (`Basis::Inferred`). A pin states
every tag, and the text does not add to it. `ink explain --why page.tags` shows the `Inline` rung
first, its joined value, and the `Front` rung beneath it as outranked. The rung stands before
`Front` because it completes `Front`'s value, not because the text outranks the frontmatter.

**Why:** one parser, one allowlist and one set of node rules for both formats; resolution in the engine, where every page is known, so both formats and both
builds share it. **Reversal:** a second dialect whose syntax comrak cannot parse, which would
need a parser of its own.

## 0044 Settings are split three ways: the site's, the operator's, and the fixed

Accepted, owner. The owner asked for the hardcoded values and limits to be configurable. The
inventory is `research/configuration.md` (97 rows). Every value is one of three kinds, and the
kind decides where it can be set:

1. **Site-owner settings**, in `ink.toml`: the image width ladder and the AVIF and JPEG quality,
   the picture `sizes` hint as a theme token, the theme's other tokens, and perhaps the length a
   description is cut at.
2. **Operator limits**, in CLI flags or the host's own configuration: the cache bound, the image
   pixel budget, workers and images opened at once, the plugin budget (fuel, time, memory, table,
   epoch tick), the git read's threads, delta cache, link hops and block, the folder read's four
   budgets, and AVIF speed. `ink.toml` may only lower an operator limit, never raise it: a hosted
   source is untrusted, and what a builder spends on it is the operator's to say. The operator's
   value is the ceiling and the site's value is `min(site, ceiling)`; a key above the ceiling is a
   diagnostic that names both numbers, not a silent clamp. The markdown format's `DEPTH` and tag
   `NAME` length are bucket 2 too and live in `plugin/format`, which this change did not touch.
3. **Never configurable**: schema and wire strings, `PREFIX`, `MARK`, `ENTRY`, `START` and
   `VERSION` constants, `Hash::DIGITS`, `SLUG_DIGITS` and the route hash digits (they change URLs
   and hashes), XML namespaces, `NONBLOCK`, `MIB` and `UNITS`, the MathML allowlist, the `OPAQUE`
   tags, the CSP and the other security headers, and the immutable `Cache-Control` value. A
   weaker value would be a hole a source could ask for, or a new identity for everything built.

The heuristic lists (content, post and template folder names, home and changelog stems,
`WRITTEN = "en"`) are inference rules. They belong to 0042's versioned rules (M4) and get no
settings of their own.

Three rules bind every setting that is added:

- **An output-affecting setting reaches the cache key, the deploy hash and the change set.** In
  the image work that is `host_image::Key`, which hashes every field of `Settings` and the pixel
  limit; a test destructures `Settings` whole, so a new field cannot be added without being
  hashed. A setting that changes output also changes the deploy hash because the output bytes
  change. A change in a setting is reported in 0042's change set as cause `config`, never
  `content`. A limit that changes no byte (workers, threads, tick) stays out of the keys.
- **New keys go through `config`'s `Value` and `Shape`.** A key is a type implementing `Value`,
  so the editor schema (architecture section 10) stays generated from one place and an
  out-of-range value is a typed error at the config boundary. The silent clamps in
  `host/image/src/encode.rs` stay as the last guard and are not a substitute.
- **A limit carries its number.** Where a limit is checked, the error holds the limit and the
  value asked for, as typed fields: `Reason::Budget { width, height, limit }`,
  `Failure::Memory { limit, wanted }`, `Failure::Table { limit, wanted }`,
  `Refused::Looped { limit, .. }`, `Error::Tick { tick }`.

**Built (2026-11-27), host side only, behavior with the defaults byte-identical:**
`host_image::Limits` (`pixels`, `workers`, `opening`), carried by `Pixels::with_limits`, with
`Settings` left holding only what changes bytes (`widths`, qualities, speed);
`host_wasmtime::Budget` gains `table` and `tick`; `host_native::GitLimits` (`threads`, `deltas`,
`hops`, `block`) is `Repository.limits`; `Disk::with_bound`. Every default equals the old value,
and every existing golden is unchanged.

**Next, after 0042's step 1 lands** (the `config/` and `cli/` code is another change's):

- `config`: an `[image]` table (`widths`, `avif-quality`, `jpeg-quality`) as `Value` types, read
  into `host_image::Settings`; a `sizes` token in `[theme]` for the quiet theme; each key
  clamped to the operator's ceiling with a diagnostic naming both numbers.
- `cli`: `Caching::pixels` builds `Pixels::cached(Settings::default(), ..)` and `Caching::new`
  builds the `Disk`; the flags `--cache-bound`, `--image-pixels`, `--workers` and `--image-speed`
  plug in there. `site.rs` builds `Repository { revision, output, .. }`, where `--git-threads`
  and its siblings plug in as `Repository.limits`. `component.rs` builds the `Runtime`, taking
  `Budget` for `--plugin-time`, `--plugin-memory` and `--plugin-fuel`. A hosted builder takes
  the same structs from its own configuration.
- `engine`: a changed setting becomes a change with cause `config` in 0042's change set; the
  description lengths wait for the owner to choose between a setting and a rule.
- `plugin/format/markdown`: `DEPTH` and the tag `NAME` length become `Budget` fields passed
  through the format call; the entity lookahead (an inline `12`) is named first.

**Why:** the values were scattered consts and literals with no one place to raise or lower
them; a split by who may set a value keeps hostile sources from spending a builder's resources
and keeps identity values out of anyone's hands. **Reversal:** a bucket 3 value turns out to
need per-site variation (the placeholder size, for instance): it moves to bucket 1 with its
field added to the cache key first.

## 0045 Releases are `dist` on version tags; the image is built from source; a rule keeps them in step

Proposed (M5); the owner confirms when they first publish. `dist` (formerly cargo-dist; its
config key and crate keep the old name), pinned at 0.34.0 in `dist-workspace.toml`, builds the
`ink` binary of `uses-ink-cli` for x86_64 and aarch64 Linux (gnu and musl), x86_64 and aarch64
macOS and x86_64 Windows, with shell and PowerShell installers and sha256 checksums. The workflow
`.github/workflows/release.yml` runs on version tags only (`pr-run-mode = "skip"`). `publish = false`
hides the binary from `dist`, so the CLI crate sets `dist = true` in `[package.metadata.dist]`;
the other crates stay unpublished (no crates.io release; `ink` is installed from the archives).
The generated workflow pins `actions/checkout@v6`; it is edited to `v7` by hand and `allow-dirty =
["ci"]` stops `dist generate --check` from refusing it, so a `dist` upgrade needs that edit again.

The self-host image is a `Dockerfile` that builds the same `dist` profile from source on musl
(`rust:<channel>-alpine`, the channel of `rust-toolchain.toml`) into a busybox runtime (busybox only
for the healthcheck's `wget`), as user 65532, with `/data` the one writable path, and runs
`ink serve --addr 0.0.0.0:8080` (0005; `serve` listens on the loopback unless told, which a
published port cannot reach, so the rule checks the address).

`cargo xtask layout` gains a `Release` rule: the dist config lists targets, exactly one crate sets
`dist = true` and has a binary, `release.yml` was generated by the version the config pins and
triggers on tags only, and the Dockerfile builds the pinned toolchain, the released package, the
released binary, and a command `ink` has. `serve` was on a `PENDING` list until it existed; the
rule fails if a command exists and is still listed.

Amended (release hardening): the rule also fails on an action not pinned to a commit hash, an
unversioned `tool:`, a workflow with no read-only top-level `permissions:`, a tag pattern with `*`,
a `gh release` script holding `${{ }}`, a release workflow with no check that the tag is on `main`,
and a `.dockerignore` that is not an allowlist covering every workspace member. The hand edits to
`release.yml` are listed in docs/release.md.

**Why:** one tool for the archives and installers, one Dockerfile with no registry login in it,
and drift between the three caught by the gate. **Reversal:** a crates.io release would drop
`publish = false` crate by crate in dependency order (the AGPL crates and the plugin guests first
need a decision on what is public).

## 0046 A source plugin sees no bytes; the host fetches, stores and measures

Accepted, as built (2026-11-27). The `ink:source` world imports `ink:host/git` and exports one
call, `read(want) -> result<snapshot, refused>`. A plugin holds the rule (how much history to
fetch for what the site needs, and a check that the snapshot is of the commit fetched); the host
holds the network, the credentials, the clone cache and every file's bytes.

- **Two calls.** `git.fetch(request)` clones into the host's cache under the builder's network
  setting, the grants it holds and the clone limits of 0044, and answers with the commit, whether
  the clone is shallow and what it cost. `git.read(cloned)` reads that commit's tree into a
  `Snapshot` with `Known` dates and the abilities observed. A refusal is a typed value (network,
  auth, budget, revision, transport) and the rest is `failed`, whose causes the host says once as
  a diagnostic.
- **Bytes stay on the host.** The files are put in a store the host made for the call and moved
  into the caller's store only when the plugin has returned a snapshot, so a plugin that fails
  or refuses leaves the caller's store as it was.
- **One budget.** The time the host spends in `git.fetch` counts against the plugin's call, and a
  source has a budget of its own, since a fetch takes longer than a page's render.
- **One rule, two bodies.** `uses-ink-source-git` is the rule over a `Git` trait of `model`, run
  natively over `host_native`'s own fetching and as a component over the imported one; a test
  holds the two to the same snapshot.

**Why:** a plugin that never holds bytes or a credential cannot leak or corrupt them, and the
budgets stay the host's. **Reversal:** a source that must transform bytes (an archive unpacker)
would need a capability that hands them over under a limit.

## 0047 Builds and tests run on a farm of slots, placed by the `farm` binary

Accepted, owner. Agents and people run the gate and the inner loop through `farm`
(docs/workflow.md), which places each job on a host of the user's config and runs it there in a
slot. `cargo xtask check` is unchanged for CI and humans.

- **A binary of its own, installed once.** `farm` is the crate `farm` at `tools/farm`, not an xtask
  task. `cargo xtask` is `cargo run -p xtask`, so every workspace and every CI job compiles
  xtask, and a farm inside it would slow CI and every task call. As a member of its own it depends
  on std, serde, serde_json and toml only (nothing of xtask or the engine, no async runtime, no
  clap) and is installed with `cargo install --path tools/farm --locked` into `~/.cargo/bin`:
  precompiled once, called by every workspace and agent as `farm ...`, which is also the one
  shape a permission rule `Bash(farm *)` has to match. `farm --version` prints the hash of the
  source it was built from (`build.rs`), and `farm status` warns when the checkout's `tools/farm`
  hashes differently, so a stale install is said and not silent.
- **It knows no project.** The jobs `farm check` runs are the repo's `.config/farm.toml`
  (`[job.<name>]` with `run`, `golden`, optional `limit`). uses.ink declares `lint`, `test`,
  `docs`, `wasm` and `sites`, with `test` and `sites` golden; the farm reads no `Stage`.

- **Slots through `flock`, no daemon.** Each host has `slots/<n>.lock` files. A job holds one
  with `flock` for as long as its command runs, so a slot is free exactly when nothing runs in
  it, and a dead job frees it. A waiting job holds a ticket file in `queue/` with its pid; the
  oldest live ticket takes the next free slot and a ticket of a dead pid is removed by whoever
  finds it. Any number of agents and sessions coordinate through the host's filesystem with no
  process of ours running between jobs. Each slot gets `cores / slots` cores for cargo and for
  nextest's test threads.
- **Hosts live outside the repo**, in `~/.config/uses-ink/farm.toml` or `$INK_FARM_CONFIG`: the
  repo is public and the names of a homelab are not part of it. No file means this machine
  alone, so the farm is always usable.
- **Reflink seeding.** A workspace's `target/` is seeded by `cp -a --reflink=always` from the
  newest warm sibling, on this machine (`farm workspace`) and on a host (the first sync), so a
  new workspace compiles its own crates and not the 800 third-party ones. Without reflinks it
  copies in full and says so.
- **Golden hosts.** Output-affecting tests and goldens run only on a golden host, by default
  x86_64, the architecture the goldens are blessed on. Other architectures take the jobs the file
  does not mark `golden`. `bless` refuses a host that is not golden.
- **The gate is fanned out by job.** `farm check` places every job of the file at once (here the
  stages CI already runs as parallel jobs), so the wall time is the slowest job and not the sum.
  What a stage runs did not change.
- **`farm wait` replaces polling.** Across 14 agents, 182 minutes went to hand-written `until
  grep` loops and 34% of wall time to calls of 20 s or more. A long job starts with `--detach`
  and one blocking `wait` (or a foreground call that passes its 540 s timeout) returns the
  result, or exit 75 and "call again". The host does the waiting, in one call.
- **A job's lifetime is its client's, unless it was let go.** A foreground job is tied to its
  connection: ctrl-c or a dropped connection stops it on the host and frees the slot. A detached
  job, or one past its timeout, is let go on purpose and ends by itself, or at `--limit`.
- **Jobs run the `ci` nextest profile.** The 100 ms budgets are for the owner's quiet machine;
  the farm's hosts are shared and a pure-logic test took 118 ms beside four agents. The farm sets
  `NEXTEST_PROFILE=ci` unless the caller chose a profile; its 5 s cap still stops a hang. The
  farm never retries a test and never changes `.config/nextest.toml`.
- **Everything is argv.** A host's program is fixed text with positional parameters; the one
  shell line, ssh's remote command, is made of `Quoted` words. Nothing from the config or the
  arguments is shell syntax. Hosts are reached with the user's own `ssh` and `rsync`, so
  credentials and the agent stay the user's.

**Why:** six agents compiling at once drove one machine to load 61 on 14 cores, 35 of 111 gate
runs failed or timed out mostly on the 100 ms budgets from CPU contention, and every workspace
compiled all of its crates cold. **Reversal:** a hosted CI-style queue, or a build cache shared
between workspaces, would replace the slot files; the verbs and the exit codes would stay.

## 0048 A page is a render unit; the theme says what it reads

Accepted. Incremental builds key each page by the hash of the engine's release, the theme's
identity (a component's hash, its tokens), the base address, the page, and the part of the site
the theme says rendering reads: `Theme::reads(page)` is `Linked` (identity, navigation, linked
pages without bodies, the assets shown), `Listed` (also every page without its body) or `Whole`,
the default. A hit is served without asking the theme. A theme with no `identity` keeps no page.
The feeds, sitemap and `robots.txt` are made every build. This refines 0006: a theme is handed
the whole site and a read through it cannot be observed, so the theme declares what it reads,
and the tests hold a declaration to what the theme does (after any one edit, an incremental
build gives the cold build's bytes, and an untouched page is not rendered again).
**Why:** the declaration is the cheap sound option; recording reads would need a lazy site
across the component boundary (0041). **Reversal:** a theme's `reads` is wrong more than once
in a release: render every page every build and key only the assets.

## 0049 Statements and schema are typed values; one renderer writes the text

Accepted. No SQL text is written in code, tests or files. A table is an identifier type (an
enum deriving `sea_query::IdenStatic`), a column is one of its variants, a statement is a
`sea_query` builder expression, a row is read back through typed columns (`Row::get(Column)`),
and the migrations are Rust values: `db::Migration` has a number, a name and the statements that
apply it, and `Steps` refuses a number that does not rise. The `db` port crate holds the builders
(pure, no driver) and `db::Renderer`, the one type that turns a statement into SQLite text with
its bound values and checks D1's limit of 100 parameters; the adapters (`db/sqlite`, later
`db/d1`) run the text and bind the values. The ledger is still `d1_migrations` with `NNNN_name.sql`
names, as wrangler writes it, so a database keeps one history; but the wrangler CLI no longer applies migrations: the
Rust runner does, through `rusqlite` self-hosted and through the D1 binding when hosted. A
`CHECK (x IN (...))` is built from the names the code reads back, so schema and code cannot
drift. `cargo xtask layout` rejects a string literal that starts with a SQL keyword, the
builders' raw escapes (`cust*`, `raw_sql`, `raw_query`) and any `.sql` file outside `spikes/`.
This reverses the "raw SQL" and "wrangler's format" of 0030 and the plain `.sql` folder of the
M3 plan; the shape of the port (statements, atomic batches, no interactive transactions) stands.
**Why:** a query that is a value is checked by the compiler, cannot be built from input, and
renders for any SQLite dialect, so D1 shares the statements. `sea-query` 1.0 is MIT OR Apache-2.0
and with the `derive` feature it adds only proc-macro crates (`sea-query-derive`, `darling`,
`heck`, `syn`, `quote`, `proc-macro2`, `thiserror`). **Reversal:** a hosted adapter that
needs a statement the builders cannot say: add the builder's missing piece to `Renderer`, not a
string to a caller.
## 0051 The edge answers from two ports and refuses what it cannot name; the adapter owns the times

Built (2026-11-27) as the serving half of M3: `edge`, `edge/axum`, `edge/folder` and `ink serve`.
The owner has not read it yet; the choices below are the ones to push back on.

- **Two ports, no host types.** `Sites` (host to the deploy's manifest) and `Blobs` (a hash, read
  in the scope `Sites` resolved: a tenant, a folder) are async traits whose futures are `Send`.
  A blob is opened first, so its length is known before a byte is read (ranges, `content-length`,
  the size budget), then read as a stream of chunks. The core owns no socket, file, clock or
  runtime; `Edge::respond` takes bytes and returns a typed `Answer`. workers-rs futures are not
  `Send`, so `edge/worker` wraps these traits in its own twin; the logic is the same code.
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

**Not done:** a `Heads` adapter for a real remote (the `gix` in the tree reads `file://` only, so
the poll has nothing to ask over HTTPS) and so the watch and the worker are built and tested but
not run by `ink serve`; serving the control plane's own deploys (no `Sites` over `Control`, the
manifest stored by the control plane has no media types), so `ink serve --data` still serves the
folder; sign-in; `ink push`; streaming a large export or upload instead of holding it in memory.
**Why:** the API is the product's one write surface, so it is typed values over ports with the
hostile input (a tar, a token, a path) refused by types that carry their data. **Reversal:** if one
call at a time is too slow, run several `Local` halves on a pool of connections; the bridge's
interface does not change.

## 0054 `ink` carries its first-party components; xtask lists them, the build script embeds them

Built (2026-11-27), closing M2's "bundle the components into `ink`". Supersedes the bullet of 0041
that reads components from a folder.

- **One list.** `cargo xtask component` (the `Guest` list of `xtask/src/component.rs`) builds every
  guest and writes `target/plugin/<name>.wasm` plus `target/plugin/bundle.json`
  (`uses.bundle/v1`): for each guest that `ink` carries, its plugin id, file, version and BLAKE3.
  `ink:format-markdown`, `ink:format-obsidian`, `ink:theme-quiet` and `ink:source-git` are carried;
  the test guest `hostile` is not. `cli/build.rs` reads `bundle.json`, checks every file against
  its hash and writes the registry (`Bundled`, a const slice of `Entry`: id, version, hash,
  `include_bytes!`) that `cli/src/bundle.rs` includes. No component is named in `cli/`: a guest
  added to xtask's list with an id is carried with no other edit.
- **The prerequisite is typed, and depends on the profile.** A release-profile build (`cargo
  install --path cli`, `dist`, the image) with no `bundle.json` fails with the command to run
  (`ink carries its components and <dir> holds none: run cargo xtask component first`), and a
  file that does not match its hash fails the same way. A debug build with none carries nothing:
  `cargo check`, clippy and the docs stage need no wasm build, and asking such an `ink` for a
  component is `Error::Unbundled` (`component.bundled.missing`) that says to run the command and
  build again. `INK_COMPONENTS` points the script at another folder. The script reruns when the
  list or a file changes, and, when there is no list yet, when the folder appears.
- **A folder can override, one component at a time, and `ink` says so.** `ink build --check` no
  longer needs `--components`. With it, a file of that folder (`<name>.wasm`, the part of the id
  after the colon) replaces the bundled component of that name; a file it lacks is the bundled
  one. Every component a run uses is a notice, `plugin.component.used` (`-v`: plugin, BLAKE3,
  `bundled` or the file), and `components` in `--json` lists id, version (unknown for a folder's
  file), hash and origin. The compiled `.cwasm` cache moved to `components/` of `--cache`, since
  it is keyed by hash and shared safely.
- **Cache keys.** A theme's `identity` is the BLAKE3 of the component's bytes and `Cargo.lock`,
  so a changed bundled theme changes every page key, and the same bytes from a folder give the
  same keys as bundled. A format has no identity: what it makes (the document) is what pages are
  keyed by, so a changed format changes the keys of the pages whose documents changed, and the
  notice carries its hash for the rest.
- **Wiring.** The Dockerfile adds the wasm target and runs `cargo --locked xtask component`
  before `cargo build`, in one step, since the cache mount holds `target/`. `dist-workspace.toml`
  sets `github-build-setup = "../build-setup.yml"`, whose one step runs `cargo --locked xtask
  component` in every build job (`release.yml` carries the same step, edited by hand like the
  checkout version, since dist 0.34 is not installed here to regenerate it). The `Release` rule
  gains two invariants: the Dockerfile runs `xtask component` before `cargo build`, and
  `release.yml` runs it. `cargo xtask check` already builds the components before the tests and
  before `sites`; `sites` now runs `ink build --check` on the bundled ones, which also tests the
  embedding.
- **Reproducibility.** The same remap flags of the Dockerfile reach the wasm build, and
  `bundle.json` records every hash, so two builds' components are compared by one file. Not
  verified: that a release workflow's runners (different hosts) write byte-identical wasm.

Measured (dist profile, `x86_64` Linux, `cli/tests/measure.rs`): `ink` is 56,590,936 bytes without
the components and 60,144,096 with them, +3,553,160 (+6.3%), of which 3,550,196 are the four
components. `ink --version` takes a median of 2.4 ms bundled and 2.8 ms unbundled, and `ink check`
of the blog fixture 3.5 ms and 3.7 ms: the embedded bytes are mapped, not read, and no difference
shows. A component is compiled on first use, as measured in 0041 (a cached load 5 to 10 ms).

**Why:** one archive or image that works with no side folder, a registry that cannot disagree with
what xtask built, and a prerequisite that fails with its own fix instead of a missing-file
error later. **Reversal:** precompiling the components at build time (`.cwasm` embedded, as the
cache stores them) would save the first compile but ties the binary to a wasmtime and CPU; it
waits for a measured first-run cost that matters.

## 0055 A source plugin fetches only its want; the host checks what it returns

Accepted. A source plugin chooses how much to fetch, not what. The host binds `git.fetch` to the
call's want (locator, revision and grant), lets `git.read` take only a clone that call fetched,
and includes the grant in the clone's cache key. A snapshot the plugin returns is accepted only
when it is of the commit the host read and names only files the host stored. The fetch is cut at
the call's wall-clock deadline, since fuel and epochs do not meter host calls. A source's terms
carry the runtime's tick or are refused. The `.cwasm` cache is sealed to the component's digest
and the engine, and is trusted as the builder's own code, not as input.

**Why:** with `network = local`, a plugin that could fetch any locator could publish a private
repository on the builder's disk. **Reversal:** a source that must fetch a repository it finds
while reading (submodules) needs a way for the want to name more than one repository.
## 0058 Review fixes of the control plane and the edge: what a failure now does

Built (2026-11-27), from a read-only review of the control plane and the serving edge.

- **A job that ended stays ended.** `claim` moves a job to `running` only if it is `queued` or
  `running` and reads the change count: a redelivered request for a job that succeeded, failed
  or was refused is `Dropped::Settled`, acked, and the job is left as it was.
- **Every failure after the claim is an attempt.** A failed deploy record or log write calls
  `give_up` like a builder failure: released for [`Control::RETRY`], then the job ends `failed`
  at `ATTEMPTS`. The ack follows the job's end, so a crash between the two is redelivered and
  dropped as settled. If `give_up` itself fails, that error is the one returned.
- **Rollback moves the pointer only from what it read.** The update adds `deploy = <read>` to
  its guard, so the event's `previous` is the deploy the pointer left; a deploy that moved it in
  between makes `rollback` read again. `deploy` (publish) still reads the pointer outside its
  batch for the event's `previous`; the pointer and the history row take it from the row, only
  the event can name a stale one. Not changed.
- **Smaller.** Blob partial names count per process, not per `Directory`. The `sql` rule flags
  `custom` as a method call or on `ColumnType` and `ColumnDef` (its doc lists what it still
  misses). `Steps::apply` refuses a ledger that is not a prefix of the set (`Unapplied::Order`).
  The sqlite adapter fails a value kind it does not know (`Unsupported::Kind`) instead of
  binding NULL. An unreadable `.git` refuses the build as `SourceRepositoryUnreadable` instead
  of falling back to plain files. `Error::Missing`, `Unreadable::Url` and `Unreadable::Revision`
  keep their source.
- **Edge.** `Limits::connections` now counts accepted connections: the accept loop takes a slot
  before it accepts, so slow heads cannot use every descriptor. What it counted before, requests
  in flight, is `Limits::requests`. `Limits::chunk` is gone: the folder adapter's chunk is its
  own constant, and no adapter read the field. `Files::open` opens the file once, checks the
  handle is the file it inspected and reads through it (a replaced name is `Error::Swapped`).
  A site folder that is a link is refused in `Folders` mode. A manifest is read through a
  `Deploys::MANIFEST_LIMIT` of 16 MiB (`Error::Large`). `dns()` checks label bytes. `ink serve`
  failures that are not the preview are `serve.run.failed`. A `Host` is lowered, not refused,
  when it is not lower case; 0051 said it was refused.

**Why:** each was a way for a failure to leave a job running forever, a store to lie, or a
budget to be a number nothing read. **Reversal:** none expected; the connection default of 1024
follows the common soft descriptor limit and is the number to change if it bites.

## 0061 The `ink` CLI has one output grammar, one renderer and a grouped command tree

Accepted by the owner after reviewing `docs/cli.md` (with `docs/cli-preview.html`); every open choice takes the recommendation in that document.
The CLI is redesigned on the model of cargo (verb gutter, `error[code]` blocks, `-q`/`-v`/`--color`),
jj (colour as named roles, global options under their own heading), vun and baudelaire (one
declarative command tree that help, completions and the manual all read; help written by ink, not
clap). In short:

- **Grammar.** Verbs for what you do to the folder in front of you (`build`, `check`, `explain`),
  a noun group where a noun has three or more verbs (`deploy list|show|rollback|resume`). `-C
  <DIR>` is one global option; the positional `<DIR>` of `build`, `check` and `preview` goes. A
  decision is addressed `[PATH] CODE`: `ink pin posts/a.md page.title`, `ink explain posts/a.md
  --why page.title`. Help has titled command groups (Write, Understand, Publish, Tools).
- **Output.** Ten typed blocks (stage line, diagnostic, notice, error, tree, ladder, change set,
  table, record, result), each both rendered as text through the theme and serialized as JSON: the
  `Shown`/`Event` pair of `report.rs` becomes the rule. One module owns colour, width, glyphs and
  TTY handling; no command builds output with `format!`. Results are stdout in one shape; the rest
  is stderr.
- **Colour.** A closed set of roles mapped once to the 8 ANSI colours, bold and faint, with no
  background, bright, 256 or truecolour, so the person's terminal theme decides the hue. Meaning
  also sits in a word or glyph; `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb` and non-TTY
  are honoured per stream.
- **Errors.** Every failure shows `error[code]`, a `caused by` rail, a typed `fix` and a `state`
  line; `uses.error/v1` gains `fix` and `state`. Snippets stay on `annotate-snippets`, styled.
- **Kept.** Every schema (`uses.build/v2`, `uses.check/v1`, `uses.explain/v2`, `uses.trace/v1`,
  `uses.select/v1`, `uses.error/v1`, `uses.event/v1`), the exit codes 0 to 6, nothing silent. New:
  `-q`, `--color`, `--no-progress`, `INK_*` variables with one precedence, `uses.progress/v1`
  events, `ink completions` (nushell included), `ink man`, `ink help exit`.

Refines `research/flow.md` section 7 (names `deploys`, `rollback`, `resume` become `deploy ...`,
and the `--json-stream` flag is dropped in favour of events under `--json`) and the "CLI output"
rules of `conventions.md`; it supersedes no decision. **Why:** today three conventions name the
site's folder, the same two global flags repeat under every command, errors are plain strings
without a fix, and the M3 verbs would each add a fourth output style. **Reversal:** a renderer that
costs more than it clarifies, or a grammar the M3 verbs do not fit: keep the typed blocks and the
colour roles, and go back to clap's help and the per-command folder. The open choices are the
last section of `docs/cli.md`.
