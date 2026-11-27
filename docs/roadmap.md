# Roadmap

Milestones in order. Each one ends with something that runs and an exit check that can fail.
No dates: a milestone is done when its exit check passes. Items marked **(owner)** need a
decision from `open.md` before they start.

## M0: Ground

The workspace, the rules, and the three unknowns answered.

- Workspace per `architecture.md` section 4: toolchain, workspace lints, `deny.toml`, nextest,
  `cargo xtask check` (the whole gate; its `layout` step checks crate names against paths,
  dependency direction, banned names).
- CI on GitHub Actions: `cargo xtask check <stage>` in five parallel jobs (`lint`, `test`,
  `docs`, `wasm`, `sites`), with the dependency build cached and the `ci` nextest profile (a
  ceiling instead of the per-test budgets); `cargo xtask check` runs them all.
- `farm` (`tools/farm`, decision 0047): the build farm client, installed once with `cargo install
  --path tools/farm --locked`; a repo's jobs are `.config/farm.toml`, hosts are the user's own
  config. Agents build, lint and test through it (docs/workflow.md).
- `token/`: one TOML file generating CSS custom properties and the pen palette.
- `pen`: own perfect-freehand port, strokes, tools, marks; golden fixtures identical on
  native and wasm32; `cargo xtask brand` regenerates the logo, mark and favicon from
  `brand/` stroke data. Delete `pen-core.js` and `gen-*.mjs` once it reaches parity; the mock's
  `pen.js` keeps only measuring and inserting, drawing through `freehand-web` (0034).
- Spikes, each a yes or no written into `decisions.md`:
  1. wasmtime host: a Rust theme component imports `pen.stroke` and `text.measure` and renders
     a page; measure instantiation and call cost with pooling and `.cwasm`.
  2. gix: shallow clone, deepen, per-file first and last commit dates for 5,000 files under 5 s.
  3. workers-rs 0.9: a Durable Object starts a Container, streams its stdout to a client, writes R2.
     Includes restricting the container's outbound network: workers-rs has no binding for
     outbound intercepts (workers-rs #1072), so the answer is an upstream PR or a small JS shim,
     written down either way.
- Submit `uses.ink` to the Public Suffix List (takes weeks; start now; steps in `psl.md`, owner sends).
- Push a fresh orphan `main` to `uses-ink/uses.ink` (owner confirmed; pushed 2026-11-26).

Progress (2026-11-26): workspace, lints, `deny.toml`, nextest and CI workflow written; `cargo xtask
check` checks names, dependency order, licences, banned names and generated files;
`token/token.toml` generates `uses-ink-token`; `freehand` ports perfect-freehand 1.2.3 and the
mock's pen core, with a golden fixture passing natively and on wasm32-wasip2; `cargo xtask brand`
draws the logo, mark and favicon from `brand/figure.toml`, byte for byte the JS output (colours now
spelled `#rrggbb`), and `gen-logo.mjs` is gone. `freehand::mark` draws the reader marks (line,
frame, ring, patch) byte for byte the mock's, and `gen-marks.mjs` is gone. `cargo xtask token` also
writes `token/token.css` (`--ink`, `--hand`, as the mocks name them). `freehand-web` is the pen in
the browser (0034); the mock draws through it and `pen-core.js` is gone. The three spikes are
answered: wasmtime (0035), gix (0036), workers-rs (0037; containers could not start under `wrangler
dev` on this host, workers-sdk #15394). Since 0038 paths are written in tenths as a `T` chain, about
ten times faster and 58% smaller; the goldens are the reference. Left: the PSL submission (owner).
Pushed as orphan `main`; old branches, tags and releases deleted from the remote (local mirror kept
outside the repo).

**Exit:** `cargo xtask check` passes in CI; the three spikes have recorded answers; the logo is
built by the pen crate.

## M1: A folder becomes a site

The engine, end to end, on the local disk.

- `model`, `config`, `diag` with the diagnostic and decision catalog.
- `engine`: inferrers for root, layout, ignore, kind, publish, title, date, slug, home, nav,
  links; `Site` built only from decisions.
- `host/wasmtime` and `host/native`; WIT for `ink:host`, `ink:format`, `ink:theme`.
- `ink:format-markdown` (comrak, math through math-core, highlighting) as a component.
- `ink:theme-quiet` as a component **(owner: its design, theme rounds 4 onward)**.
- Feeds, sitemap, 404, CSP headers in the manifest.
- CLI: `ink build <dir> -o <out>`, `ink check`, `ink explain [path]`, `ink preview` (`--pin` was
  replaced by `ink pin` in M2, 0042).
- Site fixtures: blog, flat README repo, docs folder, broken frontmatter, ambiguous links.

**Exit:** `ink build` on every fixture twice gives identical deploy hashes; no JS in any reader
page. ("`ink explain --pin` output reproduces the deploy" was dropped by 0042: it tested that
pins freeze content.)

Progress (2026-11-26): `model`, `config`, `diag`, the engine and `ink:format-markdown` run natively;
`host/native` reads a folder into a snapshot (budgets, link refusals, mtimes as bounds), stores
blobs in memory or a folder and highlights code through arborium; `ink:theme-quiet` renders every
document node through `unspliced` as one `Visit`, code through the host's highlighting and math as
MathML, as a placeholder design; the engine serves a generated
home when a site has none; the CLI has `ink build`, `check`, `explain` and `preview` with
`--json`. Site fixtures (`cli/fixture/site/`: blog, flat README repo, docs folder, broken
frontmatter, ambiguous links, a notes folder) pass the three exit tests in `cli/tests/site.rs`.
Progress (2026-11-27): `wit/` holds `ink:model`, `ink:host` (pen, text, highlight, diagnostic),
`ink:format` and `ink:theme`; `host/wasmtime` runs `ink:format-markdown` and `ink:theme-quiet` as
components through the engine's own `Format` and `Theme` traits, with fuel, time and memory
budgets and a `.cwasm` cache (decision 0041); `ink build --check` builds every CLI site fixture
natively and through the components and refuses a different deploy, and `cargo xtask check`
runs it. Bundled (decision 0054): `ink` carries `ink:format-markdown`, `ink:format-obsidian`,
`ink:theme-quiet` and `ink:source-git`, built by `cargo xtask component` and embedded by its build
script, and a `--components` folder overrides one explicitly. Left: the theme's design (owner).


## Code rework (closes M1)

The M1 crates were written before `code.md`. They are rebuilt onto its abstractions, in order,
each phase green before the next:

1. `model`: typed codes (`Code`, `Decision<C>`, `codes!`, `Value`, `FromFront`, `Toml`) and
   the document walk (`Visit`, `Fold`).
2. In parallel: `engine` onto ladders, witnesses, stages and artifacts; `config` onto `Toml`
   values; the markdown format onto `FromFront` and one builder; `freehand` onto `Tool` and
   `Mark`; `xtask` onto generators, steps and rules.
3. The theme and the CLI onto `Visit`, `Artifact` and the generic `Engine`.

**Exit:** no free function outside entry points and tests, every function name one or two
words, `cargo xtask check` enforcing both, and the M1 exit criteria still green.

## M2: Vaults, git, images

The two real audiences, locally.

- Decisions per 0042, first step: one evidence per decision, `ink explain --why`, a change set
  against the previous build with typed causes, newly public routes first, the private-source
  guard, `ink pin`.

- `ink:format-obsidian`: wikilinks, embeds, transclusion, callouts, block ids, aliases, tags,
  attachment folder from `.obsidian/`.
- Selection: `publish` frontmatter, `include`/`exclude`, `ink select` **(owner: Q16, Q17)**.
- `ink:source-git` over `git.fetch`, `Known<T>` dates, abilities observed and shown.
- Images: variants, AVIF and JPEG, dimensions, EXIF stripped, thumbhash placeholders.
- Incremental builds through the render unit index (0048); `render.cache.cold` and `render.cache.used` notices.
- `ink:theme-notebook` as a component, the pen drawing measured marks **(owner: its design)**.
- `ink build --check` in CI on every fixture.

Progress (2026-11-27): `ink:format-obsidian` is built natively and as a component (0043): a
dialect of the markdown format reading wikilinks, embeds of images and of notes (filled by the
engine, with cycles refused), Obsidian's callouts with folding, block ids, `%%` comments, inline
and frontmatter tags and `aliases`; links resolve by name in any case, by alias and by partial
path, and the attachment folder of `.obsidian/app.json` settles a shared name. `ink` reads a
folder holding `.obsidian/` as a vault through it; `cli/fixture/site/vault` builds the same both
ways. Inline tags are their own evidence (`Evidence::Inline`, joined with the frontmatter's by
the `page.tags` ladder), and `site.format` pins the format (`ink pin site.format`, `[site]
format = "ink:format-obsidian"`), with the pin, the `.obsidian/` marker and the default each
replayable by `ink explain --why site.format`.

Progress (2026-11-27): incremental builds are built (0048). A page is a render unit keyed by the
theme's identity and what `Theme::reads` says it reads; `ink build` keeps pages in `--cache`
(`host_native::Disk`) and says `render.cache.cold` or `render.cache.used`. Properties over the
fixtures and the generated blog hold that any single edit gives the deploy a cold build gives and
that an untouched page is not rendered again (`engine/tests/engine/incremental.rs`,
`cli/tests/incremental.rs`, `cli/benches/incremental.rs`). Left: the notebook theme declares its
`reads` when it lands as a component.

Progress (2026-11-27): the git source's native core is `host_native::Repository` (0036, as
built): a local repository at a commit, dates from one history walk, shallow bounds and moves
as lacks, `ink build`, `check`, `explain` and `preview` reading it when the folder holds `.git`
(`--source`, `--rev`). Update (2026-11-27): `git.fetch` (clone and deepen into a host cache, with
the limits of 0044 and typed refusals) and `ink:source-git` as a component over it are built
(0046); `host_wasmtime::Component<Source>::read` gives the same snapshot as the native core, held
by a conformance test. Left: the CLI choosing a remote `--source` through the component.

Progress (2026-11-27): 0042's first step is built. A decision is its value and one evidence;
`ink explain --why [path] <code>` replays its ladder; `ink pin [path] <code>` writes one pin and
`explain --pin` is gone; `ink build` keeps a record beside its manifest and lists the change set
against the previous build with typed causes, newly public files first, and warns on a change
ink caused; `page.publish.unstated` refuses unstated publication in a private source. The M1
exit check "pins reproduce the deploy" is now "a second build reports no change, same deploy".
Left: the change set in `ink check`, the two diagnostics of `research/inference.md` 7.2, hiding
defaults behind `ink explain --all`.

Progress (2026-11-27): selection is built (Q16, Q17). In a vault or a private source a page is
private until it is selected, by `publish: true` in its frontmatter or an `include` pattern;
`ink select` lists each page with its `page.publish` decision and evidence
(`uses.select/v1`), `ink select <path>...` writes `publish: true` into the frontmatter of the
pages, `--folder <glob>` adds an `include` entry to `ink.toml` with its comments kept, `--remove`
takes either back out and `--dry-run` writes nothing. A page with no frontmatter or an invalid one
is a typed refusal, and nothing is written when any file of the call is refused. Left: the
dashboard's selection held outside the source (Q20), which waits for hosting.

**Exit:** a real Obsidian vault and a real notes repo with history build into sites the owner
would publish; a one-file change rebuilds only its page, its neighbours and the index.

## M3: Self-host, `ink serve`

The whole product on one machine.

- `control`, ports `db`, `blob`, `queue`, `build`, `secret` with SQLite, directory and local
  adapters, conformance suites.

Progress (2026-11-27): the control plane is built and its crate gates pass. The ports `db`, `blob`,
`queue`, `build` and `secret` have their adapters (`db/sqlite`, `blob/dir`, `queue/local`,
`build/local`, `secret/local`) and conformance suites; `control` holds the use cases (accounts,
sites and handles, deploys with rollback and resume, build jobs worked off the queue,
credentials sealed at rest, export, removal with a tombstone) with tests on the real adapters.
No SQL text exists: statements and migrations are typed values (0049). Not done: `web`,
sign-in, `ink push` and the CLI verbs.

Progress (2026-11-27, 0052): the API and intake are built (`edge/api`, mounted by `edge/axum` on
its own host, `ink serve --data`), with the `Credentials` port and a single-owner token adapter,
bounded tar intake of a built folder, and the polling `watch` and in-process `worker` over `clock`
ports. Not run by `ink serve`: the watch (no remote adapter for `Heads` yet) and the worker.
`ink serve --data` does not yet serve the control plane's deploys.
- `edge` and `edge/axum`: serve, API, intake; polling watch; on-demand ACME. **Serving built
  (0051), gate green on the farm:** `edge` (ports, refusals, limits, conditional, range, 404, headers), `edge/axum`
  (hyper loop, streaming, budgets, events, `/healthz`, graceful stop), `edge/folder` and
  `ink serve`. Open: API, intake, polling watch, on-demand ACME and TLS, the worker adapter,
  redirect and variant entries in the manifest.
- `web`: dashboard (Site, What we understood, Deploys, Settings, Account) on the chosen stack,
  pen through wasm, live build log as a streamed page **(owner: dashboard stack, after the frontend
  research)**.
- Sign-in for self-host: email magic link over SMTP, plus a first-run admin link printed once.
- `ink push` and `ink:source-upload`; `ink status`, `watch`, `deploys`, `rollback`, `resume`,
  `export`.

**Exit:** on a fresh machine, `ink serve`, sign in, add a repo, see it live, push a change, see
the update, roll back, export, delete: every step with no flags and every default printed.

## M4: Hosted

- Versioned inference rules per 0042: `site.rules`, amendments, pending changes, `ink update`.

The same product on Cloudflare.

- `edge/worker`, `db/d1`, `blob/r2`, `queue/cf`, `build/container`; the per-handle Durable
  Object; deploy pipeline from CI with `wrangler`.
- `ink:authority-github` (GitHub App, webhooks, commit statuses), `ink:authority-email`,
  `ink:authority-dns`; handles with reservations and tombstones **(owner: O3, Q5)**.
- Custom domains through Cloudflare for SaaS.
- Budgets enforced and the limits page **(owner: O7, Q13)**.
- Deletion with receipt; abuse report endpoint and takedown.
- Previews of branches **(owner: Q7)**.

**Exit:** a GitHub repo goes from sign-in to live at `<handle>.uses.ink` in about a minute; a
push is live within the measured time; a custom domain goes live with a valid certificate.

## M5: Launch

- Landing at `uses.ink` from the doodle mock, on the web crate, pen through wasm.
- Docs site, built by `ink` from `docs/` with a uses.ink theme (dogfood).
- `ink:authority-forgejo` with Codeberg pre-registered; `ink:authority-gitlab`.
- Obsidian plugin and browser upload **(owner: Q18)**.
- `uses-ink/.github` profile and org description in the new brand.
- `cargo dist` releases of `ink` for Linux, macOS, Windows; container image for self-host.

**Exit:** a stranger with a repo or a vault publishes without help, and every failure they hit
shows a code, a cause and a fix.

## Later

- `ink:format-typst` once Typst's HTML export settles and its font crates are maintained.
- `ink:source-atproto` and publishing to ATproto / standard.site.
- Authority plugins as components, once the edge can host them.
- A plugin registry with signed releases.
- Opt-in reader search (pagefind; needs JS on the search page only).
