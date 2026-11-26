# uses.ink: architecture and infrastructure

Scope: how the engine and the hosted service are built, where each part runs, and the
contracts between parts. User-facing flows, states and copy live in `flow.md`; this
document uses its vocabulary (`ink push`, `ink.toml`, "What we understood", the vault
selection step). Limits and prices were read from the official docs on 2026-11-26 and are
linked inline. Anything I did not check against a source or a doc is marked `[unverified]`.

Audience, as settled by the owner: developers with a repo, and note-takers with an
Obsidian vault or a notes folder. Positioning: zero config, no lock-in.

## 1. Summary

One Rust codebase, built two ways. The **engine** (`Snapshot` in, `Site` model and output
files out) is a pure library with no I/O of its own: it asks a `Host` for everything it
needs. The **control plane** (handles, identities, sources, builds, deployments, domains)
is a second Rust library written against ports (`Db`, `Blobs`, `Queue`, `Builder`,
`Secrets`, `Clock`). Hosted, the control plane and the serving path run in a Cloudflare
Worker built with `workers-rs`, with D1 for rows and R2 for blobs, and each build runs the
native `ink` binary in a Cloudflare Container (`basic` instance, scale to zero), because
fetching a git repo with history and rendering a few thousand files does not fit a 128 MB
isolate reliably. Self-hosted, the same two libraries run in one `ink serve` process with
SQLite, a blob directory and an in-process builder. Builds are **static and
content-addressed**: every output file is stored under its BLAKE3 hash, a deployment is a
manifest of path to hash plus headers, publishing is one pointer write, rollback is another.
Sources, authorities, interpreters and renderers are plugins behind one contract whose
types are written in WIT from day one; v1 compiles first-party plugins natively against
that contract and v2 adds a wasmtime host for third-party ones without changing a type.
Every inference the engine makes is a typed `Decision` the person can read and override in
`ink.toml`; every refusal is a `Diagnostic` with a stable dotted code.

## 2. Components

```
                        readers                         authors (browser, uses CLI, Obsidian plugin)
                           |                                         |
     <handle>.uses.ink, custom hostnames (Cloudflare for SaaS)       |  api.uses.ink, app.uses.ink
                           |                                         |
  +------------------------v-----------------------------------------v---------------------+
  | Worker "edge" (Rust, workers-rs)                                                         |
  |   serve: host -> handle -> deployment -> manifest entry -> R2 object (headers from manifest)|
  |   control: accounts, handles, authorities (OAuth, DNS, DID), sources, domains, builds    |
  |   intake: webhooks (GitHub App, Forgejo, GitLab), CLI upload sessions, polling cron      |
  +-----+------------------+---------------------+-----------------------+------------------+
        |                  |                     |                       |
       D1                 R2                  Queues               Durable Object "build:<handle>"
   (facts: rows)   (blobs, snapshots,     (build requests,        (one build at a time per handle,
                    outputs, manifests,    poll ticks)              live event stream, starts the
                    event logs)                                     container)
                                                                         |
                                                       +-----------------v-----------------+
                                                       | Container: `ink build` (native)   |
                                                       |   Source plugin  -> Snapshot       |
                                                       |   Interpret      -> Site + Decisions|
                                                       |   Render         -> files + manifest|
                                                       |   Publish        -> R2 + pointer    |
                                                       +-----------------------------------+

  Self-host: every box above is one process.
  +-------------------------------------------------------------------------------+
  | `ink serve`: edge (axum) + control + intake + builder (in-process)           |
  |   ports: Db = SQLite file, Blobs = directory, Queue = in-memory + SQLite rows, |
  |          Builder = same engine call, TLS = built-in ACME (optional)            |
  +-------------------------------------------------------------------------------+

  Crates (one workspace):
    uses-model     WIT-shaped data types: Snapshot, Site, Decision, Diagnostic, events
    uses-engine    Interpret + Render, pure, talks to `Host` only
    uses-plugin    the plugin contract (generated from wit/), capabilities as a Host trait
    uses-control   handles, identities, sources, builds, deployments, against ports
    uses-edge-cf   workers-rs adapter: D1, R2, Queues, DO, Container
    uses-edge-std  tokio/axum adapter: SQLite, directory, ACME
    plugins/*      source-git, source-upload, source-atproto, source-forge-archive,
                   authority-github, authority-forgejo, authority-gitlab, authority-dns,
                   authority-atproto, authority-email, interpret-markdown, interpret-obsidian,
                   render-theme-default
    uses           the binary: build, serve, push, watch, explain, export
```

## 3. Open points

### 3.1 Runtime split

| Option | Fits a typical blog | Fits a 1 GB vault with images | History facts | Same code as self-host | Cost per build |
|---|---|---|---|---|---|
| A. Workers only (fetch + interpret + render in isolates or Workflow steps) | Yes, via archive streaming | No: 128 MB isolate, git pack buffered whole | Only through forge APIs, one request per file or batched GraphQL | Partly: engine as wasm, I/O differs | Lowest |
| B. Workers edge + Container builder | Yes | Yes, up to the instance disk | Yes, from a real clone | Yes: the same native binary | About $0.0001 to $0.0003 (below) |
| C. Own box (VPS) for builds | Yes | Yes | Yes | Yes | Fixed monthly, ops burden, single region |

Evidence:

- Workers Paid: 128 MB memory per isolate including wasm, CPU up to 5 min per request
  (default 30 s), 10,000 subrequests per invocation by default and up to 10M, 64 MiB script.
  [Workers limits](https://developers.cloudflare.com/workers/platform/limits/).
- Git in an isolate: isomorphic-git buffers the whole packfile in memory before indexing
  it (`src/commands/fetch.js` line 350, `Buffer.from(await collect(response.packfile))`,
  and the TODO at line 367 says so). A repo whose pack exceeds roughly 60 MB (128 MB minus
  the decoded copy and the runtime) fails with `exceededMemory`. A text-only blog fits; a
  vault with screenshots often does not.
- gitoxide checks three wasm targets in CI, and marks the wasm job informational
  (`DEVELOPMENT.md`: "Advisory, package-size and WASM failures remain informational").
  The plumbing crates compile; a full fetch over a Workers `fetch` transport would be our
  own transport code `[unverified that gix's HTTP transport runs on wasm32-unknown-unknown]`.
- What does fit in a Worker: streaming a forge archive (`GET /repos/{o}/{r}/tarball/{ref}`
  on GitHub, `/api/v1/repos/{o}/{r}/archive/{ref}.tar.gz` on Forgejo, confirmed in
  `templates/swagger/v1_json.tmpl`) through `DecompressionStream` and a streaming tar
  reader, hashing each file and writing it to R2. Memory is bounded by the largest file.
  Archives carry no history.
- Containers: instance types from `lite` (1/16 vCPU, 256 MiB, 2 GB) to `standard-4`;
  billed per 10 ms while running; Workers Paid includes 25 GiB-hours memory, 375
  vCPU-minutes and 200 GB-hours disk per month, then $0.0000025 per GiB-s, $0.000020 per
  vCPU-s, $0.00000007 per GB-s. [Containers limits](https://developers.cloudflare.com/containers/platform-details/limits/),
  [Containers pricing](https://developers.cloudflare.com/containers/pricing/).
  A 10 s build on `basic` (1/4 vCPU, 1 GiB, 4 GB) costs about 10 GiB-s + 2.5 vCPU-s +
  40 GB-s, so $0.00008 after the included usage, which itself covers about 9,000 such
  builds a month. Cold start time is not documented in the pages I read `[unverified,
  expect seconds]`.
- `workers-rs` 0.9.0 (released 2026-11-11) exposes D1, R2, Queues, Durable Objects with
  SQLite, and `Container` from a Durable Object (`worker/src/container.rs`,
  `State::container()` in `durable.rs`). Confirmed in a clone at `~/code/strangers/workers-rs`.
- Cloudflare Artifacts (git-compatible repos on Workers) exists but caps new repos at 1 GB
  and pushes at 64 MiB, and starts billing 2026-11-19. It is a place to host git, not a
  way to fetch someone else's repo; not used. [Artifacts limits](https://developers.cloudflare.com/artifacts/platform/limits/).

**Recommendation: B.** The Worker does what isolates are good at (routing, serving from
R2, verifying webhook signatures, OAuth redirects, short D1 writes). Every build, for every
source, runs `ink build` in a Container started by a per-handle Durable Object. One build
path for every source means one set of budgets, one event log format and one bug surface.
The CLI upload path does not need the container to ingest: the Worker commits the
snapshot from the uploaded hashes, then enqueues a build like any other.

How it collapses for self-host: `ink serve` links `uses-control` and `uses-engine`
against `uses-edge-std`. The build DO becomes a per-handle mutex, the container start
becomes a function call on a blocking thread pool, D1 becomes a SQLite file (D1 is SQLite,
so migrations are shared verbatim), R2 becomes a directory keyed by hash. Zero config:
`ink serve` with no flags uses `$XDG_DATA_HOME/uses`, port 8080, polling for watch, and
prints each of those defaults as a typed notice.

Reversal: if Container cold starts make a push-to-live time above about 15 s common, add a
fast path that runs `uses-engine` compiled to wasm inside a Workflow step for snapshots
under a size bound (the engine has no I/O, so this is an adapter, not a fork). If
Cloudflare Containers pricing or availability changes for the worse, option C runs the
same binary behind the same Queue.

### 3.2 Language and stack, and the plugin contract

| Option | Self-host single binary | Shared types edge/core | Third-party plugins later | v1 cost |
|---|---|---|---|---|
| Rust everywhere (workers-rs edge, native builder) | Yes, static | One crate | Add wasmtime host | Medium: `?Send` futures on wasm, workers-rs gaps |
| TS/Bun edge + Rust core (ink precedent) | Two artifacts, or Bun compile plus a Rust binary | Codegen from Rust to TS | Same | Low for the edge, but the control plane exists twice (Worker TS, self-host) |
| TS everywhere (Bun compile) | Bun single executable, not static | One language | JS plugins via Dynamic Workers hosted, but self-host has no equivalent sandbox | Low now, weaker sandbox story and slower render |

| Plugin contract option | Sandbox | Language-neutral | v1 work |
|---|---|---|---|
| Rust traits only | None | No | Least, but third-party later means redesign |
| WIT types, native first-party, wasm host in v2 | v2 | Yes | Small: write WIT, generate types, a lint that bans I/O outside `Host` |
| WIT and wasm components for everything in v1 (agon model) | v1 | Yes | Most: a host crate, budgets, and a git capability that crosses the boundary |
| Cloudflare Dynamic Workers (JS) | Hosted only | JS only | Self-host cannot run them the same way |

Evidence:

- The control plane is the larger half of the service (OAuth for four authorities,
  webhooks, handles, domains, deletions). Writing it once in Rust against ports is the
  agon ADR 0036 pattern; writing it in TS for the Worker and again in Rust for self-host
  breaks SSOT. ink avoided this by being TS on both hosts (`src/host/local` on Bun,
  `src/host/workers`), which works because ink has no heavy engine. uses.ink does.
- agon ADR 0042 measured wasmtime components: hostile modules contained, typed errors,
  capability checks from the import list. So the v2 path is known to work in the house.
- Dynamic Workers run arbitrary JS in fresh isolates with custom CPU and subrequest
  limits, $0.002 per unique worker per day after 1,000 a month.
  [Dynamic Workers pricing](https://developers.cloudflare.com/dynamic-workers/pricing/).
  Hosted-only, so it cannot be the contract.

**Recommendation:** Rust everywhere. Plugin contract written in WIT (`wit/uses-plugin.wit`,
package `uses:plugin@0.1`) as the single source of the boundary types; Rust types are
generated from it with `wasmtime::component::bindgen!`'s type output or `wit-bindgen`
`[unverified which generator gives clean host-side records without a wasmtime
dependency; spike it]`. In v1 every first-party plugin is a Rust crate that implements the
generated trait, depends only on `uses-plugin`, and reaches the world only through the
`Host` capabilities (`http.fetch`, `git.fetch`, `blob.read`, `clock.now`,
`diagnostic.emit`). A `clippy::disallowed_methods` and `cargo-deny` ban list keeps
`std::fs`, `std::net`, `tokio` and `reqwest` out of plugin crates, so each one also compiles
to `wasm32-wasip2` as a component. v2 adds `uses-host-wasmtime` and loads third-party
components through the same trait; first-party plugins can then ship as components too,
which is the test that no back door existed.

The `git.fetch` capability belongs to the host, not to a plugin, because it is the one
operation that needs a real disk and a native library. The git Source plugin is a thin
mapping from that capability to a `Snapshot`, so it is not special.

Edge UI: the dashboard is a client of the JSON API, out of scope here. The reader side is
static HTML with no JavaScript by default.

Reversal: if `workers-rs` blocks something the edge needs (a binding with no Rust API,
bundle size near the 64 MiB limit, startup over the 1 s budget), move only the edge
adapter to a thin TS Worker that calls the Rust control plane compiled to wasm. If the
owner wants third-party plugins at launch, the wasm host moves into v1.

### 3.3 Build model

| Option | Read latency | Freshness | Atomic publish | Rollback | Cost shape |
|---|---|---|---|---|---|
| Build-time static to R2 | One R2 read, edge cacheable | After a build | One pointer write | Pointer write | Per build |
| Request-time render | Render per miss | Immediate | No clear unit | Hard | Per read |
| Hybrid (static pages, request-time for search or feeds with query params) | Mixed | Mixed | Static part yes | Static part yes | Both |

**Recommendation: build-time static, content-addressed.** Nothing on the reader path
renders.

Keys and records:

- A **blob** is any byte string, keyed by `blake3(bytes)`. R2 key
  `t/<tenant>/b/<hash>`, scoped per tenant so deletion is exact and existence never leaks
  across tenants through dedup timing.
- A **Snapshot** is a sorted tree `path -> blob hash` plus facts, itself stored as a blob;
  its id is its hash.
- A **render unit** is one output file. Its cache key is
  `blake3(engine version, plugin versions, resolved config hash, hashes of every input it
  read)`. The engine records reads through the `Host`, so the input set is observed, not
  declared, and a page that embeds `![[other note]]` depends on that note.
- A **deployment** is a manifest `output path -> (blob hash, content type, headers)`,
  stored as a blob. Its id is its hash, so two identical builds produce the same
  deployment.
- **Publish** writes `handle.deployment = <id>` in D1 and records the previous value in
  the build record. **Rollback** writes an older id. Both are commands with events.

Incremental rebuild: the builder loads the previous build's render-unit index (cache key
to blob hash). Units whose key is unchanged are not rendered; their blob is reused. Units
that read derived site-wide data (the index, tag pages, feeds, backlinks) include the hash
of that derived slice in their key, so adding a post rebuilds the index and that post's
neighbors, not every page. The render-unit index is a cache: deleting it costs time, never
correctness, and the build log says `render.cache.cold` when it is missing.

Cache invalidation on the reader path: the edge resolves `host -> handle -> deployment id`
and then serves `GET <path>` with a Cache API key that includes the deployment id. A new
deployment changes every key, so nothing is purged. The `host -> deployment` lookup is the
only mutable read; it is cached per colo for 10 s `[unverified that 10 s is the right
trade]`, so publish emits `publish.visible_within { seconds: 10 }` and the UI says so.

Determinism: the engine sorts every collection, takes time only from facts (never from the
wall clock, the build time is a fact recorded once in the build record), and formats
numbers and dates with a fixed locale table. `ink build --check` rebuilds a recorded
deployment and compares hashes; a CI job runs it on fixtures.

Honest about SSOT: rendered output is derived data, and it is persisted because serving
needs bytes. It is treated as a cache of a pure function with a recorded input
(the build record), never edited, never read back as truth.

Reversal: request-time render becomes worth it if per-reader content appears (members,
private posts behind auth), which is not in scope.

### 3.4 Source contract

A Source turns a locator into a Snapshot. Capabilities are declared by the plugin and
then **observed** per source instance, because a GitHub source has history only when the
fetch was deep enough, and a public repo is `private: false` only until someone flips it.

| Capability | Meaning | Missing means |
|---|---|---|
| `snapshot` | Can produce a tree at a version | Not a source |
| `history` | Per-file created, updated, authors | Dates from frontmatter or filenames only; posts without either are undated; shown as a sentence (flow.md P2) |
| `watch` | Tells us about new versions (push, webhook, stream) or can be polled | Builds on manual trigger or poll; the cadence is shown |
| `private` | Needs credentials to read | Public read only |

Partial history: a fact is never a bare value. It carries its provenance and how complete
it is. A shallow clone of depth 200 cannot know when a file older than the window was
created; it knows only that it existed at the oldest commit in the window, so the fact is
a bound, not a date:

```rust
enum Known<T> {
    Exact { value: T, from: Provenance },
    NotAfter { value: T, from: Provenance },  // existed at or before this point
    Unknown { reason: UnknownReason },        // BeyondDepth, NoHistoryCapability, Renamed
}
```

Interpret decides what to do with a bound (use it with a "probably older" note, or ask
for frontmatter) and records that as a Decision. It never treats `NotAfter` as `Exact`.

Per-source behavior:

| Source | Snapshot | History | Watch | Private | Notes |
|---|---|---|---|---|---|
| Git remote (any HTTPS) | `git.fetch` in the builder, blobless partial clone then checkout `[unverified that every forge supports `filter=blob:none`; fall back to full clone, noted]` | Full, from the clone; renames via `git log --follow` equivalent in gix | Poll `info/refs` (cheap), cadence 10 min, backoff to 1 h after 24 h idle, shown as "next check at" | Via authority credentials | SSH-only hosts use `ink push` (flow.md) |
| GitHub | Same as git, authenticated with an installation token | Full | GitHub App `push` webhook, `X-Hub-Signature-256` | Installation token, 1 h lifetime ([docs](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app)) | Trees API gives blob SHAs for a cheap "what changed" check, capped at 100,000 entries / 7 MB per recursive call |
| Forgejo / Gitea | Same as git | Full | Repo webhook, HMAC-SHA256 in `X-Forgejo-Signature` and `X-Gitea-Signature` (`services/webhook/shared/payloader.go`), per-source secret | User OAuth token if the instance has our app; otherwise public only | See 3.5 |
| GitLab | Same as git | Full | Project webhook, `X-Gitlab-Token` shared secret `[unverified whether GitLab now signs payloads]` | OAuth on gitlab.com | Self-managed GitLab has the same per-instance OAuth problem |
| `ink push` (folder or vault) | Content-addressed upload: CLI sends the tree, server answers missing hashes, CLI uploads only those to R2 via presigned PUTs, server commits | None, or `mtime` as a `NotAfter` fact the CLI reports, flagged as filesystem-derived | The push is the event; `ink push --watch` pushes on change | Always private at rest | The Obsidian plugin and browser upload speak the same protocol |
| Vault in git (Obsidian Git plugin users) | As git remote | Full | As git | As git | Detected by `.obsidian/` at the root |
| ATproto repo | `com.atproto.sync.getRepo` CAR or `listRecords` for known collections, mapped to virtual paths `at/<collection>/<rkey>.json` | `createdAt` from records as a claimed fact (author-asserted), commit `rev` as version | Jetstream filtered by `wantedDids` and `wantedCollections` from a Durable Object holding an outbound WebSocket; fallback poll `com.atproto.sync.getLatestCommit` per DID | Public by protocol | Jetstream was rewritten as a full-network archive and replay service; the filter parameters still exist (`cmd/client/loadtest.go`) |
| Forge archive (no git) | Stream tarball, no clone | None | As the forge | As the forge | Used for previews of unclaimed public repos (flow.md 2.4): cheap, no history, marked as such |

Change detection without a full fetch: a source may report native ids (git blob OIDs from
a trees API) as facts. The builder keeps a per-source map `native id -> blake3` and skips
downloading blobs it already has. The Snapshot is always keyed by our hash; the forge's id
is a fact, never the key.

Credentials:

- GitHub: no per-user secret is stored. The App private key is a Worker secret; the edge
  mints an installation token per build, passes it to the container in the start request
  body (never env, never disk), and the container drops it at exit.
- Forgejo/GitLab user OAuth: the refresh token is the only stored credential, sealed with
  AES-256-GCM under a key from Cloudflare Secrets Store `[unverified Secrets Store binding
  in workers-rs; else a Worker secret]`, AAD = source id, so a row copied to another source
  fails to open. Access tokens are minted per build and never written.
- Self-host: the sealing key is a `0600` file in the state dir, created on first run with
  a notice; `ink serve` refuses to start if its mode is wider (`secret.key.mode`).
- The CLI holds a uses.ink session token from an OAuth device grant (RFC 8628), 30 day
  refresh, stored in the OS keyring `[unverified keyring crate support on all targets]`.

### 3.5 Authority and ownership

An Authority proves that an identity controls something: an account, a repo, a domain, a
DID. Proofs are typed and dated; a handle's right to a source is the proof, not the URL.

| Authority | Proves | Mechanism | Per-instance setup |
|---|---|---|---|
| GitHub | Account, and admin on chosen repos | GitHub App install (requires repo admin) plus user OAuth for identity | One App for uses.ink; self-hosters register their own |
| Codeberg | Account, repos | OAuth2 app registered once on codeberg.org | One registration |
| Self-hosted Forgejo/Gitea | Account or repo write | See options below | The problem |
| GitLab.com | Account, repos | OAuth app | One |
| DNS | Control of a domain (custom domains, bare git URL hosts) | TXT `_uses-ink.<name>` = handle-bound token | None |
| Repo challenge | Write access to one repo, any git host | Commit a file `.well-known/uses-ink` containing a handle-bound token, or put it in `ink.toml`; read anonymously | None |
| ATproto DID | Control of a DID | atproto OAuth; the client id is a URL to our client metadata document, so there is no registration step `[unverified current spec details: DPoP and PAR required]` | None |
| Email | Control of an address | Magic link, 15 min, single use | None |

Self-hosted Forgejo and Gitea, checked in source (`~/code/strangers/forgejo`,
`~/code/strangers/gitea`):

- **No dynamic client registration.** Neither codebase implements RFC 7591 (no
  `registration_endpoint` in code), and the live discovery documents at
  `https://codeberg.org/.well-known/openid-configuration` and
  `https://gitea.com/.well-known/openid-configuration` advertise none.
- **Built-in public clients exist** (`models/auth/oauth2.go`): `git-credential-oauth`,
  `git-credential-manager`, `tea`, `fj` (Forgejo) with fixed client ids and loopback
  redirects, and the port is ignored for loopback per RFC 8252 (`ContainsRedirectURI`). A
  `ink` CLI could run PKCE against any default instance with the `git-credential-oauth`
  id. I recommend against it: the consent screen names another tool, and admins can turn
  the list off with `[oauth2] DEFAULT_APPLICATIONS`.
- **Any user can register their own OAuth2 app** in user settings `[unverified that every
  default config allows it; it is the Gitea default]`.

| Option for self-hosted Forgejo/Gitea | Proves | Private repos | UX cost |
|---|---|---|---|
| Repo challenge file | Write to the repo | No | One commit |
| Instance admin registers uses.ink once; we store `(instance, client id, sealed secret)` as an authority config row | Account | Yes | Zero for users after the admin step |
| User registers an app in their own settings and pastes id and secret | Account | Yes | High |
| Borrow a built-in client id | Account | Yes | Low, but misleading |

**Recommendation:** the repo challenge as the universal baseline (works for any git URL,
including ones with no forge at all), per-instance app registration as an authority config
any admin can add through a form (Codeberg pre-registered), and user-registered apps as the
fallback for private repos on instances whose admin did not. Webhook creation needs either
OAuth or the user pasting our webhook URL and secret; we show both.

Handles:

- A handle is a lowercase DNS label, 3 to 32 characters, `[a-z0-9-]`, no leading or
  trailing hyphen, no `--` (reserved for future `<site>--<handle>` previews, flow.md Q4).
- Claiming needs one verified identity. The forge username is offered first when free.
  Names collide across forges, so it is first claim wins, with no forge having priority.
- Reserved: infrastructure (`www`, `app`, `api`, `docs`, `status`, `mail`, `admin`,
  `static`, `cdn`, `assets`, `d`), anything matching our own route nouns, and a short
  list of well-known brands to reduce phishing `[owner: how long a brand list]`.
- Renames and deletions leave a tombstone: the old name redirects then stays unclaimable
  for the period in flow.md Q5, so a released name cannot be reused to phish an old
  audience.
- Abuse: a report endpoint per site, a takedown command that sets the pointer to a
  tombstone deployment (content kept for appeal, recorded as an event), and build rate
  limits per handle and per identity.

### 3.6 Zero-config Interpret

Interpret is a list of **inferrers**, each a plugin of the same kind, run in a fixed
order. Each one reads the Snapshot and the decisions made before it, and returns new
decisions and diagnostics. The Site model is built only from decisions, so every value in
it has a cause.

```
detect-root -> detect-layout (blog | vault | docs | flat) -> ignore -> classify
  (home, post, page, asset, draft, unpublished) -> metadata (title, date, summary, tags)
  -> links (markdown links, wikilinks, embeds) -> nav -> derived (index, tags, feeds,
  backlinks)
```

Inference rules for v1 (each line is one decision code):

| Decision | Inputs, in order of precedence | Default when nothing matches |
|---|---|---|
| `root` | the one folder of `content`, `docs`, `notes`, `src/content` present, when it holds every file a format reads (repository files at the top aside). Not `posts` or `blog`: they are layout markers, and a root there would make every post a plain page | repo root |
| `layout` | `.obsidian/` present = vault; root `docs` = docs; a top `posts`/`blog`/`writing` folder or dated filenames = blog | flat |
| `ignore` | dotfiles and dot folders (`.obsidian/`, `.trash/`, `.git/`, `.github/`), `node_modules`, `ink.toml` at the top; at the top of the root, files whose whole stem is `LICENSE`/`LICENCE` (or `LICENSE-MIT`, a licence id in capitals after `-` or `.`), `CHANGELOG`, `CONTRIBUTING`, `CODE_OF_CONDUCT`, `SECURITY`, in any case; in a vault, a top `templates/` folder in any case (the conventional name: `.obsidian/templates.json` is not read yet). Each folder or file ignored this way says so (`file.ignore.inferred`) | none |
| `home` | `index.md`, `README.md`, `Home.md`, `Index.md` at root | a generated listing |
| `kind` | frontmatter `type`, a top posts folder (its own `index.md` or `README.md` is a listing page), frontmatter `date`, a dated filename | page |
| `publish` | frontmatter `publish: true/false`, `draft: true` (`draft: false` says nothing, so it never selects a vault's note), an `include` pattern, a `drafts/` folder anywhere or a name starting with `_`; for vaults and private sources, the selection step (flow.md 2.6) | public source: published; vault or private source: not published until selected |
| `title` | frontmatter `title`, first H1, filename | filename |
| `date` | frontmatter `date`, filename `YYYY-MM-DD-*`, history `created` if `Exact` | undated (and said so) |
| `updated` | frontmatter `updated`, history `updated` | none |
| `slug` | frontmatter `slug`, filename lowercased and hyphenated | |
| `asset` | referenced by a published page | not published (privacy: an attachment no published note uses stays private) |
| `link.resolve` | relative path; for wikilinks, Obsidian's rule: exact path, then unique basename, then shortest path | unresolved: rendered as plain text, `link.unresolved` |
| `link.private` | target exists but is not published | rendered as plain text without the target's title, `link.to_unpublished` |

Obsidian-specific handling:

- `[[Note]]`, `[[Note|alias]]`, `[[Note#Heading]]`, `[[Note#^block-id]]` resolve to pages
  and anchors; an ambiguous basename is a diagnostic listing the candidates, never a guess.
- `![[image.png]]`, `![[image.png|300]]` become images; `![[Note]]` and
  `![[Note#Heading]]` transclude the target's rendered section, and the embedding page's
  render key includes the target's hash.
- The attachments folder is read from `.obsidian/app.json` (`attachmentFolderPath`
  `[unverified key name]`) and used only to resolve embeds, so a vault with attachments in
  `assets/` and one with them next to notes both work.
- Callouts (`> [!note]`), inline `#tags`, and frontmatter `tags`/`aliases` are parsed;
  aliases join link resolution.
- Backlinks, tag pages and the link graph are derived from the published set only, every
  build, and never stored as facts.

Recording and overriding: a Decision has a stable code, the subject (a path or the site),
the chosen value, the evidence that chose it, the alternatives it rejected, and the
`ink.toml` key that pins it. "What we understood" (flow.md section 3) is a view over the
decision list. `ink explain --pin` prints the decisions as a `ink.toml` that reproduces
them.

Config file: **`ink.toml`** at the source root, matching flow.md.

| Format | Parser with spans | Editor schema | Familiar to the audience | Precedent |
|---|---|---|---|---|
| TOML | `toml_edit`/`toml` with spans | Taplo, JSON Schema | Developers (Cargo, Hugo, Zola) | agon |
| KDL | `kdl` crate v6 with spans | Thin | Few | baudelaire |
| YAML | Many, inconsistent | Good | Note-takers (frontmatter) | none |

TOML wins on tooling and audience; KDL's advantage (nested nodes) is not needed for a file
of overrides. Unknown keys are a diagnostic with a suggestion, never ignored. A config
value that overrides an inference turns that Decision's evidence into `Config { span }`.

Reversal: if real vaults show that people want per-folder rules often, add `[[rule]]`
tables with glob matchers, still in one file.

### 3.7 Custom domains

Evidence ([plans](https://developers.cloudflare.com/cloudflare-for-platforms/cloudflare-for-saas/plans/),
[getting started](https://developers.cloudflare.com/cloudflare-for-platforms/cloudflare-for-saas/start/getting-started/),
[hostname validation](https://developers.cloudflare.com/cloudflare-for-platforms/cloudflare-for-saas/domain-support/hostname-validation/),
[apex proxying](https://developers.cloudflare.com/cloudflare-for-platforms/cloudflare-for-saas/start/advanced-settings/apex-proxying/)):

- Cloudflare for SaaS on a Free zone: 100 custom hostnames included, $0.10 each after,
  50,000 max. Wildcard custom hostnames are Enterprise only.
- A Worker can be the fallback origin: an originless record (`AAAA 100::`) as fallback and
  a `*/*` Workers route on the zone catches custom hostnames and our own subdomains.
- A custom hostname must not equal the zone name, so `uses.ink` itself is never one.
- Apex proxying (static IPs so customers can use an A record) is an Enterprise paid add-on.
  Without it, an apex works only where the customer's DNS flattens CNAMEs (Cloudflare DNS,
  and ALIAS/ANAME at some providers `[unverified list]`).
- Hostname validation (`status`) and certificate validation (`ssl.status`) are separate;
  both must be `active`. Real-time validation passes once the CNAME points at us;
  pre-validation by TXT avoids downtime when moving from another host.

| Option | Apex | Cost | Work |
|---|---|---|---|
| Cloudflare for SaaS, CNAME to `domains.uses.ink` | With CNAME flattening only | $0.10/host beyond 100 | Low |
| Same, plus apex proxying | Yes | Enterprise contract | Contract |
| Our own ACME at the edge | No (Workers cannot terminate TLS for arbitrary hosts) | | Not possible hosted |

**Recommendation:** Cloudflare for SaaS with CNAME target `domains.uses.ink`. Binding a
hostname to a handle needs our own TXT proof (`_uses-ink.blog.example.com`, the DNS
authority above) before we create the custom hostname, so a CNAME left pointing at us
cannot be adopted by another handle. For apex domains the UI checks for flattening and,
when it is absent, says so and offers `www` plus a redirect the customer sets at their
registrar. Each state from the Cloudflare API (`pending`, `pending_validation`, `active`,
`moved`, error codes) maps one to one to a typed domain state.

Self-host: `ink serve --tls acme` uses on-demand ACME (HTTP-01) only for hostnames
present in the domains table, so a random SNI cannot make it request certificates. Without
it, the binary serves plain HTTP behind the operator's proxy.

Reversal: apex proxying becomes worth buying if apex requests dominate support.

### 3.8 Isolation and safety

| Risk | Control |
|---|---|
| One site reads another's cookies or storage | Each handle is its own origin; `uses.ink` is submitted to the [Public Suffix List](https://publicsuffix.org/) (as `github.io` is) so `alice.uses.ink` and `bob.uses.ink` are different sites to browsers. The dashboard lives at `app.uses.ink`, which the PSL entry also makes its own site. Reader pages set no cookies. PSL inclusion takes weeks `[unverified current turnaround]`, so request it before launch |
| Script on a reader page | No JS by default. Markdown raw HTML is parsed and sanitized with `ammonia` against an allowlist; every removed element or attribute is a `html.stripped` diagnostic with its line. CSP on every page: `default-src 'none'; img-src 'self' https: data:; style-src 'self'; font-src 'self'; media-src 'self' https:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'`, emitted by Render into the manifest headers |
| Untrusted SVG | Served only as a file, never inlined, with `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox` and `Content-Type: image/svg+xml`, so opening it directly runs nothing. Referenced through `<img>`, where browsers do not run SVG script |
| Embeds (video, code playgrounds) | v1: none beyond images and links. Later: an allowlist of providers rendered as sandboxed iframes, each a Render plugin |
| Builder escape | Containers isolate per instance; one build per container start; no inbound network; outbound only to the source host and our API with a build-scoped token `[unverified outbound restriction options for Containers; the docs mention an outbound handler]` |
| Resource exhaustion | Budgets below, each a typed refusal with the number and the limit |
| Phishing and malware hosting | No JS, no forms (`form-action 'none'`), file types allowlisted (no executables, no HTML uploaded as an asset), report endpoint, takedown event |

Budgets (v1 defaults, shown on the limits page flow.md Q13 asks for):

| Budget | Value | Code when exceeded |
|---|---|---|
| Fetched source size | 1 GiB | `source.budget.bytes` |
| Files in snapshot | 20,000 | `source.budget.files` |
| Single file | 25 MiB | `source.budget.file_bytes` |
| Published output | 500 MiB | `render.budget.bytes` |
| Build wall time | 120 s | `build.budget.time` |
| Builder memory | instance size (1 GiB) | `build.budget.memory` |
| Builds per handle per hour | 30 | `build.budget.rate` |

### 3.9 Storage

Facts (persisted):

| Store | What | Notes |
|---|---|---|
| D1 / SQLite | `account`, `identity (authority, subject, verified_at)`, `handle`, `handle_identity`, `source (kind, locator, observed capabilities)`, `authority_config (instance, client id, sealed secret)`, `credential (sealed refresh token)`, `proof`, `build (inputs, status, deployment id)`, `deployment_pointer`, `domain`, `event` | Small rows, one migration set for both |
| R2 / directory | blobs, snapshots, deployment manifests, build event logs (NDJSON), render-unit indexes | Keyed by hash under `t/<tenant>/` |

Derived (never persisted as truth): the Site model, decisions, backlinks, tag pages, nav,
feeds, the "What we understood" view. They are recomputed from the Snapshot and config.
Render output and render-unit indexes are caches with recorded inputs (3.3).

Garbage collection: a blob is live if reachable from a deployment pointer, the last N
deployments kept for rollback (`[owner: N, default 20]`), or a snapshot of a pending
build. A sweep marks from those roots and deletes the rest, per tenant.

Export (`ink export`, and the button in flow.md 5.8): a tarball with the latest
snapshot's files as uploaded (for CLI sources, which have no other home), the rendered
deployment, `ink.toml` as pinned, and every build record and event log as NDJSON.

Deletion: unpublish first (pointer to tombstone), then delete R2 objects under the tenant
prefix, then rows, following ink's rule that bytes go before the row so a crash leaves a
diagnosable dangling row. Custom hostnames are removed from Cloudflare in the same job.
The result is an event with counts, and a receipt to the account email.

### 3.10 Observability

A build is an append-only list of typed events, the same on every surface: streamed live
from the build Durable Object over SSE, stored as NDJSON in R2 at the end, printed by
`ink build` and `ink watch` in the terminal, summarized in a commit status on forges
that have one.

```rust
enum BuildEvent {
    Started { build: BuildId, trigger: Trigger, inputs: BuildInputs },
    SourceFetched { snapshot: SnapshotId, files: u32, bytes: u64, capabilities: Observed },
    Decided(Decision),
    Diagnosed(Diagnostic),
    Defaulted { setting: SettingKey, value: Value, reason: DefaultReason }, // nothing silent
    Retried { step: Step, attempt: u8, cause: Diagnostic },
    Rendered { units: u32, reused: u32, bytes: u64 },
    Published { deployment: DeploymentId, previous: Option<DeploymentId>, visible_within_s: u16 },
    Refused(Diagnostic),
    Finished { status: BuildStatus, wall_ms: u32 },
}
```

Operator-side metrics (build durations, container starts, cache hit rate) go to Workers
Logs hosted and `tracing` to stderr self-hosted. They are not the user's log; the user's
log never contains another tenant's data or our internals.

## 4. Core type sketches

Sketches, not code. Names follow the house ban list. All are WIT-expressible: records,
variants, lists, options, no closures.

```rust
// ---------- identity of content ----------
struct Hash([u8; 32]);                       // blake3
struct SnapshotId(Hash);

struct Snapshot {
    source: SourceId,
    version: Version,                        // git commit, upload id, atproto rev
    tree: Vec<Entry>,                        // sorted by path, unique
    facts: Vec<FileFacts>,                   // optional, per path
    observed: Observed,                      // what this fetch actually had
}
struct Entry { path: ContentPath, blob: Hash, size: u64, native: Option<NativeId> }
enum Version { GitCommit(Oid), Upload(UploadId), AtprotoRev(String) }

struct FileFacts {
    path: ContentPath,
    created: Known<Instant>,
    updated: Known<Instant>,
    authors: Known<Vec<Author>>,
}
enum Known<T> { Exact { value: T, from: Provenance }, NotAfter { value: T, from: Provenance }, Unknown { reason: UnknownReason } }
enum Provenance { GitHistory { commit: Oid }, ForgeApi { endpoint: Endpoint }, Filesystem, RecordClaim { field: FieldName } }
enum UnknownReason { NoHistoryCapability, BeyondDepth { depth: u32 }, NotReported }

// ---------- capabilities ----------
enum Capability { Snapshot, History, Watch(WatchMode), Private }
enum WatchMode { Webhook { signature: SignatureScheme }, Stream { endpoint: Endpoint }, Poll { every_s: u32 }, Push }
struct Declared(Vec<Capability>);            // what the plugin can do
struct Observed { has: Vec<Capability>, lacks: Vec<Lack> }  // what this instance did
struct Lack { capability: Capability, consequence: DecisionCode }

// ---------- sources and authorities ----------
trait Source {                               // generated from wit/uses-plugin.wit
    fn describe(&self) -> SourceDescription; // kind, Declared, locator schema
    fn probe(&self, host: &dyn Host, locator: &Locator, grant: Option<Grant>) -> Result<Observed, Diagnostic>;
    fn snapshot(&self, host: &dyn Host, locator: &Locator, at: Option<Version>, grant: Option<Grant>) -> Result<Snapshot, Diagnostic>;
    fn version(&self, host: &dyn Host, locator: &Locator, grant: Option<Grant>) -> Result<Version, Diagnostic>; // cheap poll
}
struct Grant { token: Sealed<Token>, expires: Instant }     // short-lived, never logged

trait Authority {
    fn describe(&self) -> AuthorityDescription;              // kind, what it can prove
    fn begin(&self, host: &dyn Host, claim: Claim) -> Result<Challenge, Diagnostic>;   // OAuth URL, TXT record, file token
    fn verify(&self, host: &dyn Host, claim: Claim, response: ChallengeResponse) -> Result<Proof, Diagnostic>;
}
enum Claim { Account { instance: Instance }, Repo { locator: Locator }, Domain { name: DomainName }, Did { did: Did }, Email { address: Email } }
struct Proof { claim: Claim, subject: Subject, method: ProofMethod, verified_at: Instant, recheck_after: Option<Instant> }

// a forge package is one plugin providing both
struct Forge { authority: AuthorityDescription, source: SourceDescription }

// ---------- interpretation ----------
struct Decision {
    code: DecisionCode,                      // "post.date"
    subject: Subject,                        // Site | Path(ContentPath)
    chosen: Value,
    evidence: Evidence,
    rejected: Vec<Alternative>,
    pin: ConfigKey,                          // where to override it in ink.toml
}
enum Evidence { Frontmatter { path: ContentPath, span: Span }, Filename { path: ContentPath }, History { fact: Provenance }, Layout { marker: ContentPath }, Config { span: Span }, Default { reason: DefaultReason } }

struct Site {
    root: ContentPath,
    layout: Layout,                          // Blog | Vault | Docs | Flat
    home: PageRef,
    pages: Vec<Page>,
    assets: Vec<Asset>,
    nav: Vec<NavItem>,
    unpublished: Vec<Excluded>,              // path + the decision that excluded it
}
struct Page { id: PageRef, path: ContentPath, route: Route, kind: PageKind, title: String, date: Option<Instant>, updated: Option<Instant>, tags: Vec<Tag>, aliases: Vec<String>, body: Document }
enum PageKind { Home, Post, Page, Index { of: Collection } }
// Document is a typed tree (headings, paragraphs, links with resolved targets,
// embeds, callouts); links point at PageRef or Anchor, never at strings.
// Backlinks, tag pages and feeds are functions of Site, not fields.

// ---------- output ----------
struct Deployment { entries: Vec<Served> }   // id = hash of the manifest
struct Served { route: Route, blob: Hash, content_type: MediaType, headers: Vec<Header> }

// ---------- diagnostics ----------
struct Diagnostic {
    code: DiagnosticCode,                    // "source.git.auth", dotted, stable
    severity: Severity,                      // Error | Warning | Note
    fault: Fault,                            // Author | Source | Operator | Ours
    subject: Option<Subject>,
    span: Option<Span>,
    context: Vec<(ContextKey, Value)>,       // typed, worded by the catalog
    fix: Option<Fix>,                        // a config key, a command, a link
    cause: Option<Box<Diagnostic>>,          // chain
}
```

## 5. Proposed ADRs

1. Rust for engine, control plane, edge and CLI; one workspace, one binary `ink`.
2. Hosted runtime: Worker edge (workers-rs) plus one Container per build; no fetch or render in isolates in v1.
3. Self-host is `ink serve`, one process, SQLite plus a blob directory, zero flags required, every default announced.
4. The engine has no I/O; it calls a `Host` whose reads are recorded and become render cache keys.
5. Plugin boundary types are written in WIT (`uses:plugin@0.1`); v1 plugins are native crates against those types; v2 adds a wasmtime host.
6. `git.fetch` is a host capability; the git Source is a plugin over it.
7. Content addressing with BLAKE3, blobs scoped per tenant, no cross-tenant dedup.
8. Builds are static; a deployment is a hashed manifest; publish and rollback are pointer writes.
9. Facts carry provenance and completeness (`Known<T>`); partial history is a bound, never a date.
10. Capabilities are declared by plugins and observed per source; every lack maps to a stated consequence.
11. Authorities are separate from sources; proofs are typed and dated; a forge package provides both.
12. Self-hosted Forgejo/Gitea: repo challenge baseline, per-instance app registration as data, no borrowed client ids.
13. Interpret is an ordered list of inferrers that emit Decisions; the Site model is built only from Decisions.
14. Vaults and private sources publish nothing until the person selects; unreferenced attachments are never published.
15. One config file, `ink.toml` at the source root; unknown keys are diagnostics.
16. Custom domains via Cloudflare for SaaS with our own TXT binding to a handle; apex only with CNAME flattening.
17. `uses.ink` on the Public Suffix List; dashboard on `app.uses.ink`; reader pages have no JS, no cookies, a strict CSP written by Render.
18. Budgets are typed refusals with numbers, published on a limits page.
19. Deletion removes bytes before rows and ends with a receipt.
20. The build log is a typed event stream, identical in the terminal, the browser and storage.

## 6. Questions only the owner can answer

| # | Question | My default |
|---|---|---|
| O1 | Should reader pages ever run author JavaScript (a script tag in a post, a custom theme script)? It decides the CSP and how much abuse surface we host. | Never in v1 |
| O2 | Third-party plugins at launch, or after the contract has settled with first-party ones? | After; the wasm host is v2 |
| O3 | Brand and squatting policy for handles: how long a reserved brand list, and are idle handles ever reclaimed? | Short list; never reclaim a handle with a verified identity |
| O4 | Is per-instance OAuth registration for self-hosted Forgejo a form any instance admin can submit, or something we approve by hand? | Any admin, verified by an admin-only API call on that instance `[unverified that such a call exists]` |
| O5 | Apex domains: accept "use `www` plus a redirect" for customers without CNAME flattening, or pay for apex proxying? | Accept the limit, say it plainly |
| O6 | Rollback history depth kept in storage (cost vs. how far back rollback goes). | 20 deployments |
| O7 | Free tier budgets: are the numbers in 3.8 the free tier, or the only tier? Is there a paid tier at all? | One tier at launch |
| O8 | For vaults, is the selection step per note (Obsidian Publish style) or per folder, and does the choice live in frontmatter (`publish: true`) so it travels with the note? | Frontmatter, with folder-level select as a bulk action that writes nothing to the vault and is stored as a `ink.toml` `include` the CLI or plugin writes locally `[conflicts with flow.md Q10 for git sources; owner call]` |
| O9 | Should the hosted service ever render a public repo its owner has not claimed (flow.md Q1)? This sets whether the forge-archive source is exposed at all. | Previews only, 24 h, unlisted |
