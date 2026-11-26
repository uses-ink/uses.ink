# Roadmap

Milestones in order. Each one ends with something that runs and an exit check that can fail.
No dates: a milestone is done when its exit check passes. Items marked **(owner)** need a
decision from `open.md` before they start.

## M0: Ground

The workspace, the rules, and the three unknowns answered.

- Workspace per `architecture.md` section 4: toolchain, workspace lints, `deny.toml`, nextest,
  `xtask check` (crate names against paths, dependency direction, banned names).
- CI on GitHub Actions: fmt, clippy, nextest, `cargo deny`, wasm32 build of the pen.
- `token/`: one TOML file generating CSS custom properties and the pen palette.
- `pen`: own perfect-freehand port, strokes, tools, marks; golden-hash fixtures identical on
  native and wasm32; `cargo xtask brand` regenerates the logo, mark and favicon from
  `brand/` stroke data. Delete `pen-core.js`, `pen.js`, `gen-*.mjs` once it reaches parity.
- Spikes, each a yes or no written into `decisions.md`:
  1. wasmtime host: a Rust theme component imports `pen.stroke` and `text.measure` and renders
     a page; measure instantiation and call cost with pooling and `.cwasm`.
  2. gix: shallow clone, deepen, per-file first and last commit dates for 5,000 files under 5 s.
  3. workers-rs 0.9: a Durable Object starts a Container, streams its stdout to a client, writes R2.
     Includes restricting the container's outbound network: workers-rs has no binding for
     outbound intercepts (workers-rs #1072), so the answer is an upstream PR or a small JS shim,
     written down either way.
- Submit `uses.ink` to the Public Suffix List (takes weeks; start now; steps in `psl.md`, owner sends).
- Push a fresh orphan `main` to `uses-ink/uses.ink` **(owner: confirm before pushing)**.

Progress (2026-11-26): workspace, lints, `deny.toml`, nextest and CI workflow written; `cargo xtask
check` checks names, dependency order, licences, banned names and generated files;
`token/token.toml` generates `uses-ink-token`; `freehand` ports perfect-freehand 1.2.3 and the
mock's pen core, with a golden fixture passing natively and on wasm32-wasip2; `cargo xtask brand`
draws the logo, mark and favicon from `brand/figure.toml`, byte for byte the JS output (colours now
spelled `#rrggbb`), and `gen-logo.mjs` is gone. `freehand::mark` draws the reader marks (line,
frame, ring, patch) byte for byte the mock's, and `gen-marks.mjs` is gone. `cargo xtask token` also
writes `token/token.css` (`--ink`, `--hand`, as the mocks name them). `freehand-web` is the pen in
the browser (0034); the mock draws through it and `pen-core.js` is gone. Left: the three spikes, the
PSL submission. Pushed as orphan `main`; old branches, tags and releases deleted from the remote
(local mirror kept outside the repo).

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
- CLI: `ink build <dir> -o <out>`, `ink check`, `ink explain [--pin] [path]`, `ink preview`.
- Site fixtures: blog, flat README repo, docs folder, broken frontmatter, ambiguous links.

**Exit:** `ink build` on every fixture twice gives identical deploy hashes; `ink explain --pin`
output, put back as `ink.toml`, reproduces the same deploy; no JS in any reader page.

## M2: Vaults, git, images

The two real audiences, locally.

- `ink:format-obsidian`: wikilinks, embeds, transclusion, callouts, block ids, aliases, tags,
  attachment folder from `.obsidian/`.
- Selection: `publish` frontmatter, `include`/`exclude`, `ink select` **(owner: Q16, Q17)**.
- `ink:source-git` over `git.fetch`, `Known<T>` dates, abilities observed and shown.
- Images: variants, AVIF and JPEG, dimensions, EXIF stripped, thumbhash placeholders.
- Incremental builds through the render unit index; `render.cache.cold` notice.
- `ink:theme-notebook` as a component, the pen drawing measured marks **(owner: its design)**.
- `ink build --check` in CI on every fixture.

**Exit:** a real Obsidian vault and a real notes repo with history build into sites the owner
would publish; a one-file change rebuilds only its page, its neighbours and the index.

## M3: Self-host, `ink serve`

The whole product on one machine.

- `control`, ports `db`, `blob`, `queue`, `build`, `secret` with SQLite, directory and local
  adapters, conformance suites.
- `edge` and `edge/axum`: serve, API, intake; polling watch; on-demand ACME.
- `web`: dashboard (Site, What we understood, Deploys, Settings, Account) on the chosen stack,
  pen through wasm, live build log as a streamed page **(owner: dashboard stack, after the frontend
  research)**.
- Sign-in for self-host: email magic link over SMTP, plus a first-run admin link printed once.
- `ink push` and `ink:source-upload`; `ink status`, `watch`, `deploys`, `rollback`, `resume`,
  `export`.

**Exit:** on a fresh machine, `ink serve`, sign in, add a repo, see it live, push a change, see
the update, roll back, export, delete: every step with no flags and every default printed.

## M4: Hosted

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
