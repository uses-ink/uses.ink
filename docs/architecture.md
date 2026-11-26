# Architecture

The spec for uses.ink: what each part does, where it runs, and the contracts between parts.
Vocabulary and naming are in `conventions.md`, decisions with their reasons in `decisions.md`,
dependencies in `libraries.md`, the build order in `roadmap.md`, owner questions in `open.md`.
Evidence (limits, prices, source reads) is in `research/`; this page states what we build.

## 1. In one paragraph

One Rust workspace builds one binary, `ink`. A **source** plugin turns a locator into a
content-addressed **snapshot**. The **engine** interprets the snapshot through **format** plugins
into a typed `Site` where every inferred value is a recorded **decision**, then renders it
through a **theme** plugin into files. The files and a manifest are stored by BLAKE3 hash; a
**deploy** is that manifest, and publishing is one pointer write. Builder-side plugins (source,
format, theme) are WebAssembly components run by wasmtime and reach the world only through host
**capabilities**; the pen that draws every hand-drawn mark is one of those capabilities. Hosted,
a Cloudflare Worker serves sites and runs the control plane, and each build runs `ink build` in a
Cloudflare Container. Self-hosted, `ink serve` is the whole service in one process.

## 2. Pipeline

```
source plugin --> Snapshot --> Interpret --> Site + Decisions --> Render --> files + Manifest --> Publish --> Serve
   (component)    (hashed)     (engine +       (typed, every        (theme       (hashed blobs)    (pointer    (edge reads
                                format          value has a          component)                     write)      manifest)
                                components)     cause)
```

| Stage | Input | Output | Pure | Runs in |
|---|---|---|---|---|
| Snapshot | locator, credential grant | `Snapshot` (sorted tree of path to blob hash, facts, observed abilities) | no, through `git`/`http` capabilities | builder |
| Interpret | snapshot, `ink.toml` | `Site`, `Vec<Decision>`, `Vec<Diagnostic>` | yes | builder |
| Render | site, theme, tokens | files, `Manifest` | yes | builder |
| Publish | manifest | pointer write, `deploy.published` | no | control plane |
| Serve | host, path | bytes and headers from the manifest | read-only | edge |

Pure means: no clock, no randomness, no environment, no I/O except recorded host reads. The same
inputs give the same bytes, natively or in wasm, today or next year.

## 3. Runtime shapes

```
Hosted                                                   Self-hosted
--------------------------------------------------       ------------------------------------
Worker "edge" (workers-rs)                               ink serve (tokio + axum)
  serve   host -> handle -> live deploy -> blob             same edge crate, axum adapter
  api     control plane commands and views              db      SQLite file
  intake  webhooks, upload sessions, poll cron          blob    directory keyed by hash
  web     landing and dashboard                         queue   SQLite rows + in-process worker
D1 (rows)  R2 (blobs)  Queues (build requests)          build   same builder, blocking pool
Durable Object build:<handle>                           tls     optional on-demand ACME
  one build at a time, live log as a streamed page
  starts the Container
Container: ink build (native binary, wasmtime inside)
```

One builder binary for every source. The Worker never fetches a repo and never renders.

## 4. Repository layout

```
uses.ink/
  Cargo.toml  rust-toolchain.toml  rustfmt.toml  clippy.toml  deny.toml  .config/nextest.toml
  model/                 uses-ink-model         Snapshot, Site, Page, Document, Decision, Diagnostic, Event, Manifest
  config/                uses-ink-config        ink.toml: parse with spans, pins, schema for editors
  diag/                  uses-ink-diag          diagnostic and notice catalog (catalog.toml), wording, terminal rendering
  engine/                uses-ink-engine        interpret and render, pure, talks to host traits only
  host/                  uses-ink-host          capability traits: blob, git, http, pen, text, asset, diagnostic
  host/wasmtime/         uses-ink-host-wasmtime component loading, linking, fuel, epochs, pooling, .cwasm cache
  host/native/           uses-ink-host-native   real capabilities for the builder: gix, http client, blob store, highlighting
  freehand/              freehand               the pen: strokes, tools, marks; native and wasm32
  freehand-web/          freehand-web           the pen as plain wasm exports, and its loader freehand.js
  measure/               (own name)             text shaping and measuring with real fonts
  control/               uses-ink-control       accounts, handles, identities, sources, builds, deploys, domains
  db/  db/d1/  db/sqlite/  db/conformance/       row port and adapters
  blob/  blob/r2/  blob/dir/  blob/conformance/  blob port and adapters
  queue/  queue/cf/  queue/local/                build request port
  build/  build/container/  build/local/         builder port: start a build, stream its events
  secret/                uses-ink-secret        sealing credentials at rest
  edge/                  uses-ink-edge          serve, api, intake, independent of the HTTP runtime
  edge/worker/           uses-ink-edge-worker   workers-rs entry point, bindings
  edge/axum/             uses-ink-edge-axum     axum entry point
  web/                   uses-ink-web           landing and dashboard (stack in section 14)
  cli/                   uses-ink-cli           the `ink` binary
  plugin/
    sdk/                 uses-ink-plugin-sdk    guest-side helpers, published, MIT OR Apache-2.0
    source/     wit/  git/  upload/  archive/  atproto/
    authority/  wit/  github/  forgejo/  gitlab/  dns/  email/  atproto/
    format/     wit/  markdown/  obsidian/  typst/
    theme/      wit/  quiet/  notebook/
  wit/host/              ink:host               capability interfaces every plugin may import
  migration/             plain .sql, applied by wrangler to D1 and by ink serve to SQLite
  token/                 design tokens (one TOML file) -> CSS custom properties and pen palette
  brand/                 logo stroke data, fonts and their licences
  fixture/               uses-ink-fixture       the fixture harness: goldens through insta; a dev-dependency only
  xtask/                 cargo xtask token | diag | brand | layout | check (later: wit)
  docs/  spikes/
```

Dependency direction, enforced by `cargo xtask layout` (a step of `cargo xtask check`) from
`cargo metadata`:

```
model <- config, diag <- engine <- host <- host/{wasmtime,native} <- cli
model <- control <- edge <- edge/{worker,axum}; control <- db, blob, queue, build, secret (traits only)
plugin/* <- plugin/sdk, wit only. Never engine, control, host/* or edge.
freehand, measure <- nothing of ours. freehand-web <- freehand. token <- nothing of ours.
fixture <- nothing of ours; every crate may take it as a dev-dependency, none as a normal one.
```

## 5. Core types

Sketches; WIT is the source for anything that crosses the plugin boundary, Rust types are
generated from it. Full sketches with commentary: `research/architecture.md` section 4. The
types as built live in `model/`; where they differ from these sketches, `model/` is right:
a decision is typed by its code (`Decision<C>`, erased as `AnyDecision` for storage and the
wire; `code.md` section 1), and a diagnostic's code, severity, fault and typed fields are one generated enum (`Problem`) from
`diag/catalog.toml`.

```rust
struct Hash([u8; 32]);                                   // blake3
struct Snapshot { source: SourceId, version: Version, tree: Vec<Entry>, facts: Vec<FileFacts>, observed: Observed }
struct Entry { path: ContentPath, blob: Hash, size: Bytes, native: Option<NativeId> }
enum Version { GitCommit(Oid), Upload(UploadId), AtprotoRev(Rev) }

struct FileFacts { path: ContentPath, created: Known<Instant>, updated: Known<Instant>, authors: Known<Vec<Author>> }
enum Known<T> { Exact { value: T, from: Provenance }, NotAfter { value: T, from: Provenance }, Unknown { reason: UnknownReason } }

enum Ability { Snapshot, History, Watch(WatchMode), Private }
struct Observed { has: Vec<Ability>, lacks: Vec<Lack> }  // Lack carries the decision it affects

struct Decision<C: Code> { subject: C::Subject, value: C::Value, evidence: Evidence, rejected: Vec<Alternative<C::Value>> }
// the pin is derived from code + subject, never stored

struct Site { layout: Layout, home: PageId, pages: Vec<Page>, assets: Vec<Asset>, nav: Vec<NavItem>, excluded: Vec<Excluded> }
struct Page { id: PageId, file: ContentPath, route: Route, kind: PageKind, title: Text, date: Option<Instant>, updated: Option<Instant>, tags: Vec<Tag>, body: Document }
// Document is a typed tree; links point at PageId or Anchor, never strings.
// Backlinks, tag pages and feeds are functions of Site, not fields.

struct Manifest { entries: Vec<Served> }                 // deploy id = hash of the manifest
struct Served { route: Route, blob: Hash, media: MediaType, headers: Vec<Header> }

struct Diagnostic { code: DiagnosticCode, severity: Severity, fault: Fault, subject: Option<Subject>,
                    span: Option<Span>, context: Vec<(ContextKey, Value)>, fix: Option<Fix>, cause: Option<Box<Diagnostic>> }
enum Fault { Author, Source, Operator, Ours }
```

## 6. Plugins

Four slots. Every first-party plugin uses the public contract; there is no private API.

| Slot | Does | Runs as | Runs in | Main capabilities |
|---|---|---|---|---|
| `source` | locator to snapshot; cheap version poll | wasm component | builder | `git`, `http`, `blob`, `diagnostic` |
| `format` | one file's bytes to a `Document`, frontmatter facts and decisions | wasm component | builder | `blob`, `diagnostic` |
| `theme` | pages to files (contract in section 7) | wasm component | builder | `pen`, `text`, `highlight`, `asset`, `diagnostic` |
| `authority` | begin and verify a claim, mint a grant | native crate against the generated WIT types | edge | `http`, `dns`, `clock`, `secret` |

Authorities stay native in v1 because the edge is a Worker, which cannot host wasmtime. They use
the same WIT types, so moving them to components later changes no type.

**Host capabilities** (`wit/host/`, package `ink:host`):

| Interface | Functions | Notes |
|---|---|---|
| `blob` | `read(hash)`, `put(bytes) -> hash` | every read is recorded; reads become render cache keys |
| `git` | `fetch(locator, depth, grant)`, `deepen`, `log(paths)` | host-side gix; the only operation needing a real disk |
| `http` | `fetch(request)` | allowlisted hosts per plugin manifest; no other network |
| `pen` | `stroke`, `mark`, `palette` | section 8 |
| `text` | `measure(font, size, text)`, `lines(font, size, text, width)` | real font shaping, so marks fit their words |
| `highlight` | `highlight(language, code) -> result<list<run>, unhighlighted>` | classed runs from tree-sitter grammars in the host (decision 0039); natively `model::highlight::Highlight`, implemented by `host_native::Grammars` |
| `asset` | `theme(path)`, `slot(name)` | a theme's bundled files; a slot already rendered from the author's `theme/` override |
| `diagnostic` | `emit(diagnostic)`, `notice(notice)` | typed; plugins never print |

No plugin imports a clock, randomness, environment or filesystem. A plugin's manifest declares
the capabilities it imports; linking fails with `plugin.capability.undeclared` otherwise.

**Budgets per call**: fuel and an epoch deadline per plugin call, memory per instance from the
pooling allocator, all typed refusals (`plugin.budget.fuel`, `plugin.budget.memory`,
`plugin.budget.time`). Components are precompiled to `.cwasm` once per plugin version and cached.

**Identity and loading**: a plugin is `<scope>:<slot>-<variant>` at a version, pinned by digest in
`ink.lock` next to `ink.toml` (written by the CLI, optional for first-party defaults). v1 loads
first-party plugins bundled in the binary and third-party ones from a URL or path pinned by
digest. A registry is later.

## 7. Themes

A theme is a wasm component that implements `ink:theme/theme`. The engine owns the site's
structure; the theme renders pages; a theme may declare extra routes (decision 0014).

```wit
package ink:theme@0.1.0;

world theme {
  import ink:host/pen@0.1.0;
  import ink:host/text@0.1.0;
  import ink:host/highlight@0.1.0;
  import ink:host/asset@0.1.0;
  import ink:host/diagnostic@0.1.0;
  use ink:model/site@0.1.0.{site, page-id, route, file};

  export describe: func() -> theme-info;                       // name, tokens it accepts, page kinds it renders
  export assets: func(tokens: tokens) -> list<file>;           // css, fonts, images, once per build
  export render: func(page: page-id, site: borrow<site>, tokens: tokens) -> result<html, diagnostic>;
  export routes: func(site: borrow<site>) -> list<route-request>;      // declared extra routes, may be empty
  export render-route: func(route: route, site: borrow<site>, tokens: tokens) -> result<file, diagnostic>;
}
```

`site` is a host-owned resource with methods (`pages`, `page`, `backlinks`, `tags`, `children`),
so a theme reads only what it needs and every read is recorded into the page's cache key.

**What the engine owns** and no theme can change: routes of pages, feeds (Atom and RSS), sitemap,
`robots.txt`, image variants, the CSP and every header, link resolution, the 404 page's status.
**What a theme owns**: the HTML of each page, its CSS, fonts and images, and any extra routes it
declares. Extra routes go through the same manifest; a collision with an engine route or another
extra route is `theme.route.collision`, never an overwrite.

**Customizing, in layers**, all in the author's source:

| Layer | Author does | Mechanism |
|---|---|---|
| 0 | nothing | `ink:theme-quiet` |
| 1 | set tokens | `[theme] accent = "#2b59c3"`; tokens are typed by the theme's `describe()`; an unknown token or bad value is a diagnostic |
| 2 | add CSS | `theme/custom.css`, appended after the theme's CSS |
| 3 | override a named slot | a theme declares slots in `describe()` (`header`, `footer`, `post-meta`); the author's `theme/<slot>.html` is rendered by the host with minijinja against the typed page data and handed to the theme |
| 4 | another theme | `theme = "bob:theme-zine@1.2.0"`, pinned in `ink.lock` |

First-party themes: `ink:theme-quiet` (default, calm, the author's voice) and
`ink:theme-notebook` (the hand-drawn one, opt-in). Their design is still open (`open.md`).

## 8. The pen

One crate, `freehand` (decision 0033), draws every hand-drawn mark in the product: reader themes (through the `pen`
capability), the dashboard and landing (compiled to wasm32), the logo and favicons (`cargo xtask
brand`), social preview images (rasterized with resvg). There is no second implementation.

- Deterministic: a mark is a function of its seed, its geometry and the pen version. RNG is
  mulberry32 seeded by an FNV hash of the seed string; trig goes through `libm`, so native and
  wasm produce identical bytes. Golden fixtures (the full path data) run on both targets. Paths
  are written in tenths of a pixel as a `Q` then a `T` chain (0038).
- Tools: `Pencil` (MS Paint pencil: thin, nearly constant width, small tremor) and `Highlighter`
  (smooth jittered fill, offset by the shift), each a type with its parameters. The red note pen
  is the pencil in the `pen` colour.
- Strokes: low-frequency perpendicular jitter, overshoot and bowing at ends, closed loops drawn
  as two arcs, outlines from our own port of perfect-freehand's `getStroke`.
- Marks built on strokes (`freehand::mark`), each a type implementing `Mark`: `Line` (rules),
  `Underline`, `Frame` (boxes), `Ring`, `Fill`, `Patch` (tape, highlights), `Drawn` (any
  centreline); `Tick` and `Arrow` when a theme needs them.
  A mark takes measured geometry (from `text.measure` or the DOM), so it fits the thing it marks.
- Colours come from `token/`, the same file that produces the CSS custom properties, so a colour
  exists once. The pen takes no colour: its caller paints each path (`freehand` depends on
  nothing of ours).
- Animation (draw-in, swipe-in fills, boil on hover) is data the mark carries (stroke order,
  path length); the dashboard animates it, reader themes ignore it.

## 9. Interpret

An ordered list of inferrers, each reading the snapshot and earlier decisions and returning
decisions and diagnostics. The `Site` is built only from decisions, so every value has a cause.

```
site.root -> site.layout -> file.ignore -> file.format -> page.kind / page.publish -> page.title,
page.date, page.updated, page.slug, page.tags -> link.resolve -> site.home, site.nav -> derived (index, tags, feeds, backlinks)
```

The v1 inference table (inputs in order of precedence, default when none match) is
`research/architecture.md` section 3.6; it moves into `diag/catalog.toml` and the decision
codes as the inferrers are written, and this page links there once it exists.

Rules that hold for every inferrer:

- A `Known::NotAfter` fact is never treated as `Exact`; using it is its own decision with a note.
- Ambiguity is a diagnostic listing the candidates (`link.target.ambiguous`), never a guess.
- Vaults and private sources publish nothing until selected; an attachment no published page
  uses is never published (decision 0017).
- A pinned value turns the decision's evidence into `Config { span }`, shown as "you said so".

## 10. `ink.toml`

One file at the source root. Optional. Every key is either a pin of a decision code, the theme
choice with its tokens, or a file pattern. Built in `config/` (`uses-ink-config`). A sketch:

```toml
include = ["notes/2023.md"]          # top-level keys go before the first table
exclude = ["notes/private/**"]

[theme]
use = "ink:theme-quiet"              # slot choice; omitted means the default, recorded as a notice
accent = "#2b59c3"                   # every other key is a token the chosen theme declared

[site]
title = "Ann's notes"                # pins site.title
description = "Notes on books."      # pins site.description; false pins none
authors = ["Ann"]                    # pins site.authors; always an array, first author first
language = "fr-CH"                   # pins site.language, a BCP 47 tag
home = "README.md"                   # pins site.home; false pins a generated listing
nav = ["about.md"]                   # pins site.nav

[file."drafts/x.md"]
ignore = true                        # pins file.ignore

[page."posts/old/x.md"]              # per-file pins
title = "On shelves"                 # pins page.title
summary = "Books, and where they go." # pins page.summary; false pins none
```

The table and key come from the decision code's name: a sitewide code `t.k` is `[t] k`, a per-file
one is `[t."<path>"] k`. A key is read once its code exists in `model`, so rules such as a posts
folder or draft folders join as decision codes, not as config keys of their own. TOML allows a
key only once, so the theme is a `[theme]` table with `use`, not a `theme = "..."` string beside
it. Pattern syntax (`*`, `?`, `**`; a pattern matching a folder matches what is in it) is
documented in `config/src/pattern.rs`.

`ink explain --pin` prints the whole file that reproduces the current decisions; `--minimal`
prints only what differs from the defaults. The CLI writes pins with `toml_edit`, preserving the
author's formatting and comments. A JSON schema for editor completion through Taplo is not
generated yet; it has to come from the decision codes, not from a second set of config types.

## 11. Build, storage, serving

- **Blob**: any byte string at `blake3(bytes)`, under `t/<tenant>/b/<hash>`. Scoped per tenant:
  no cross-tenant dedup, exact deletion, no timing leak.
- **Render unit**: one output file, cache key `blake3(engine version, plugin digests, resolved
  config, hashes of every recorded read)`. Unchanged keys reuse their blob. A missing cache is
  `render.cache.cold`, a notice.
- **Deploy**: the manifest, stored as a blob, id = its hash. Identical builds give the same deploy.
- **Publish** writes `site.live = <deploy>` and records the previous value; **rollback** writes an
  older one. Both are commands with events. Rollback pauses automatic publishing until `resume`.
- **Serve**: `host -> site -> live deploy -> entry -> blob`, headers from the manifest. The edge
  cache key includes the deploy id, so publishing purges nothing. The host-to-deploy lookup is
  cached per colo for at most 10 s, and `deploy.published` says `visible-within: 10 s`.
- **Retention**: blobs reachable from the live deploy, the last 20 deploys, and pending builds;
  a per-tenant sweep removes the rest.
- **Rows** (D1 or SQLite, one migration set): `account`, `identity`, `handle`, `handle_identity`,
  `source`, `authority_config`, `credential` (sealed), `proof`, `build`, `site_live`, `domain`,
  `event`.
- **Derived, never stored as truth**: the `Site`, decisions, backlinks, feeds, "what we
  understood". Rendered output is a cache of a pure function with a recorded input.

## 12. Sources, authorities, domains

| Source | Snapshot | History | Watch | Private |
|---|---|---|---|---|
| `ink:source-git` (any HTTPS git, GitHub, Forgejo, GitLab) | `git.fetch`, shallow then deepen | from the clone | webhook where the forge has one, else poll `info/refs` every 10 min, backing off to 1 h | with a grant |
| `ink:source-upload` (`ink push`, Obsidian plugin, browser upload) | content-addressed upload: send tree, upload missing hashes | none; `mtime` as `NotAfter` | the push is the event | always private at rest |
| `ink:source-archive` (forge tarball) | streamed archive | none | as the forge | as the forge |
| `ink:source-atproto` (later) | repo records as virtual files | record claims | Jetstream | public |

| Authority | Proves | Mechanism |
|---|---|---|
| `ink:authority-github` | account, admin on chosen repos | GitHub App plus user OAuth; installation token per build, never stored |
| `ink:authority-forgejo` | account, repos | OAuth per instance; Codeberg pre-registered; any instance admin can register us; repo challenge as baseline |
| `ink:authority-gitlab` | account, repos | OAuth |
| `ink:authority-dns` | a domain | TXT `_uses-ink.<name>` with a handle-bound token |
| `ink:authority-email` | an address | magic link, 15 min, single use |
| repo challenge (built into source-git) | write to one repo | a committed `.well-known/uses-ink` or a token in `ink.toml` |

Custom domains: Cloudflare for SaaS, CNAME to `domains.uses.ink`, after our own TXT proof binds
the name to a handle. Apex only where the customer's DNS flattens CNAMEs; otherwise `www` plus
a redirect, said plainly. Self-host: on-demand ACME only for names in the domain table.

Handles: lowercase DNS label, 3 to 32 chars, no `--`, first claim wins across forges, reserved
infrastructure and route names, tombstones on rename and delete.

## 13. Safety and budgets

- `uses.ink` on the Public Suffix List, so every handle is its own site to browsers; the
  dashboard is `app.uses.ink`.
- Reader pages: no JavaScript, no cookies, no forms. CSP written by the engine into every
  manifest entry: `default-src 'none'; img-src 'self' https: data:; style-src 'self'; font-src
  'self'; media-src 'self' https:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'`.
- Raw HTML in content goes through our own allowlist into `Document`; every removed element is
  `html.element.stripped` with its line. Untrusted SVG is served only as a file with a sandbox CSP.
- Builder: one build per container start, outbound only to the source host and our API with a
  build-scoped credential.
- Budgets, each a typed refusal with the number and the limit, shown on a public limits page:

| Budget | v1 value | Code |
|---|---|---|
| fetched source | 1 GiB | `source.budget.bytes` |
| files in snapshot | 20,000 | `source.budget.files` |
| one file | 25 MiB | `source.budget.file` |
| published output | 500 MiB | `render.budget.bytes` |
| build wall time | 120 s | `build.budget.time` |
| builds per handle per hour | 30 | `build.budget.rate` |
| plugin call | fuel, 64 MiB, 2 s | `plugin.budget.*` |

## 14. Surfaces

**Reader**: static HTML from the theme, section 7.

**Dashboard** at `app.uses.ink` (`/` on self-host): five screens from `research/flow.md` section
8: Site, What we understood, Deploys, Settings, Account. It writes nothing to the author's source
and holds no interpretation settings; those live in `ink.toml`. Live build logs are a streamed
page: the dashboard writes the page head, then each log line as the build's Durable Object (or
the local builder) emits it, then the tail (`unspliced::Split`). Stack (0032): an axum router served by workers-rs hosted and by `ink serve`;
HTML, SVG and CSS from `unspliced`; behaviour is native HTML, forms and view transitions, so
there is no hand-written or generated JS except the pen's loader (`freehand-web/freehand.js`, 0034);
colours and sizes from `token/`; the pen as a wasm module.

**Landing** at `uses.ink`: from `.impeccable/mocks/doodle/index.html`, served by the same web
crate.

**CLI** (`ink`): `login`, `logout`, `whoami`, `init`, `build`, `check`, `explain`, `preview`,
`push`, `watch`, `status`, `deploys`, `rollback`, `resume`, `source`, `domain`, `handle`,
`theme`, `select`, `export`, `account`, `diag`, `serve`. Output rules, samples and exit codes:
`research/flow.md` section 7, which moves here when the CLI is built. Every command takes
`--json`; streaming commands emit NDJSON events with a `schema` field.

## 15. Events

A build is an append-only list of typed events, identical in the terminal (`ink build`,
`ink watch`), the dashboard (a streamed page), storage (NDJSON in the blob store) and forge commit statuses.

```rust
enum BuildEvent {
    Started { build: BuildId, trigger: Trigger, inputs: BuildInputs },
    SourceFetched { snapshot: SnapshotId, files: u32, bytes: Bytes, observed: Observed },
    Decided(AnyDecision),
    Diagnosed(Diagnostic),
    Noticed(Notice),                         // defaults chosen, caches cold, retries
    Rendered { units: u32, reused: u32, bytes: Bytes },
    Published { deploy: DeployId, previous: Option<DeployId>, visible_within: Duration },
    Refused(Diagnostic),
    Finished { status: BuildStatus, wall: Duration },
}
```

Operator metrics go to Workers Logs hosted and `tracing` on stderr self-hosted. The author's log
never contains another tenant's data or our internals.
