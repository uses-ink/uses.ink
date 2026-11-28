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
- `crates/core/token/`: one TOML file generating CSS custom properties and the pen palette.
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
`crates/core/token/token.toml` generates `uses-ink-token`; `freehand` ports perfect-freehand 1.2.3 and the
mock's pen core, with a golden fixture passing natively and on wasm32-wasip2; `cargo xtask brand`
draws the logo, mark and favicon from `brand/figure.toml`, byte for byte the JS output (colours now
spelled `#rrggbb`), and `gen-logo.mjs` is gone. `freehand::mark` draws the reader marks (line,
frame, ring, patch) byte for byte the mock's, and `gen-marks.mjs` is gone. `cargo xtask token` also
writes `crates/core/token/token.css` (`--ink`, `--hand`, as the mocks name them). `freehand-web` is the pen in
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
- `crates/runtime/host/wasmtime` and `crates/runtime/host/native`; WIT for `ink:host`, `ink:format`, `ink:theme`.
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
`crates/runtime/host/native` reads a folder into a snapshot (budgets, link refusals, mtimes as bounds), stores
blobs in memory or a folder and highlights code through arborium; `ink:theme-quiet` renders every
document node through `unspliced` as one `Visit`, code through the host's highlighting and math as
MathML, as a placeholder design; the engine serves a generated
home when a site has none; the CLI has `ink build`, `check`, `explain` and `preview` with
`--json`. Site fixtures (`crates/surface/cli/fixture/site/`: blog, flat README repo, docs folder, broken
frontmatter, ambiguous links, a notes folder) pass the three exit tests in `crates/surface/cli/tests/site`.
Progress (2026-11-27): `wit/` holds `ink:model`, `ink:host` (pen, text, highlight, diagnostic),
`ink:format` and `ink:theme`; `crates/runtime/host/wasmtime` runs `ink:format-markdown` and `ink:theme-quiet` as
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

Layout and monolith rework (0074, Accepted; `docs/layout.md`): `crates/` by layer, the `cli`, `control`,
`model`, `farm` and `xtask` splits, one file per decision, in phases that each pass `farm check`.
**(owner: accepted; work packages in `docs/layout.md` section 13)**

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
folder holding `.obsidian/` as a vault through it; `crates/surface/cli/fixture/site/vault` builds the same both
ways. Inline tags are their own evidence (`Evidence::Inline`, joined with the frontmatter's by
the `page.tags` ladder), and `site.format` pins the format (`ink pin site.format`, `[site]
format = "ink:format-obsidian"`), with the pin, the `.obsidian/` marker and the default each
replayable by `ink explain --why site.format`.

Progress (2026-11-27): incremental builds are built (0048). A page is a render unit keyed by the
theme's identity and what `Theme::reads` says it reads; `ink build` keeps pages in `--cache`
(`host_native::Disk`) and says `render.cache.cold` or `render.cache.used`. Properties over the
fixtures and the generated blog hold that any single edit gives the deploy a cold build gives and
that an untouched page is not rendered again (`crates/core/engine/tests/engine/incremental.rs`,
`crates/surface/cli/tests/incremental.rs`, `crates/surface/cli/benches/incremental.rs`). Left: the notebook theme declares its
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
`queue`, `build` and `secret` have their adapters (`crates/port/db/sqlite`, `crates/port/blob/dir`, `crates/port/queue/rows`,
`crates/runtime/build/process`, `crates/port/secret/local`) and conformance suites; `control` holds the use cases (accounts,
sites and handles, deploys with rollback and resume, build jobs worked off the queue,
credentials sealed at rest, export, removal with a tombstone) with tests on the real adapters.
No SQL text exists: statements and migrations are typed values (0049). Not done: `web`,
sign-in, `ink push` and the CLI verbs.

Progress (2026-11-27, 0066): sign-in is built. `control::signin` (typed `Credential` with the
email link as its only variant, digests of 256-bit tokens at rest, single-use links, sliding or
absolute sessions with revocation, the once-only first-run link, the sign-up policy), the `mail`
port with `Capture`, `Stdout` and the SMTP adapter `crates/port/mail/smtp`, rate limits, and the API routes
`POST /v1/login`, `GET` and `POST /v1/login/{token}`, `GET` and `DELETE /v1/session` in
`crates/server/edge/api`'s `Door`, beside the owner token. `ink serve --data` prints the first-run link once
and takes the `--mail`, `--smtp-*`, TTL, policy and rate flags. Not done then: the dashboard screens
(`web`), a route to list and revoke sessions, a forwarded-header source, passkeys. Built since
(2026-11-28): sessions are listed and ended by id with an Account screen (0082), the Origin check and
the challenge limit (0083), step-up (0084), recovery codes (0085) and the forwarded-header source
(0086), with their `ink serve` flags (`--allow-origin`, `--challenge-limit`, `--recovery-limit`,
`--step-up-window`, `--forwarded-hops`), migration 203 and a recovery codes section on the Account
screen. Open: the email recovery with a delay, the mail notice on a recovery code's use, a label on a
session, and the edge's mounting of `Screens` (nothing implements it yet, so `end_session` has no
implementation to write).

Progress (2026-11-28, 0092): `ink serve --data` serves the dashboard on the API host. `Screens` is implemented over the
control plane (`crates/surface/cli/src/selfhost/dash`): sign-in by email link and passkey, the site list, a site, its
deploys, builds and stored logs, rollback and rebuild, settings, export, and the account with sessions, passkeys and
recovery codes, every change a `POST` behind the Origin check. `--origin` is derived from the listen address when not
given, and `--site-ttl` defaults to 0. `crates/surface/cli/tests/dashboard.rs` runs it through the real binary. Not done:
live logs for a running build, the email recovery, a replay for `why`.

Progress (2026-11-27, 0052): the API and intake are built (`crates/server/edge/api`, mounted by `crates/server/edge/axum` on
its own host, `ink serve --data`), with the `Credentials` port and a single-owner token adapter,
bounded tar intake of a built folder, and the polling `watch` and in-process `worker` over `clock`
ports. Not run by `ink serve`: the watch (no remote adapter for `Heads` yet) and the worker.
`ink serve --data` does not yet serve the control plane's deploys.

Progress (2026-11-27, 0067): `ink serve --data` runs the loop. It serves the control plane's
deploys by host (`crates/server/edge/control`), runs the watch and the worker as supervised threads with typed
events, remembers the watch's last-seen commit, reads remote heads with `gix` (`crates/server/watch/git`) and
builds git sources through the `ink:source-git` component. `crates/surface/cli/tests/selfhost.rs` adds a
`file://` repository, sees it served, pushes, sees the update, rolls back, exports and deletes,
over the real binary with no network. `ink push` and export followed (0077, below); streaming intake is not done.
Git over HTTPS followed (0072): the watch's head read and the source fetch go through one rustls and
`ring` client, with a licence exception for `webpki-root-certs` alone, tested against a local TLS server.
Progress (2026-11-27, 0077): `ink init`, `push`, `status`, `watch` and `export` are built over the
signed API: a tree is posted, the blobs the site lacks go up in bounded tar batches (resume is
running the push again), the build is followed and the deploy printed, with exit codes 3, 4 and 5.
Intake stays whole-body within the `tree` and `part` limits. `crates/surface/cli/tests/push.rs` runs all of it over
the real binary. Not done: live build stages for `watch` (the log is stored when a build ends),
domains in `status`, streaming intake.
- `edge` and `crates/server/edge/axum`: serve, API, intake; polling watch; on-demand ACME. **Serving built
  (0051), gate green on the farm:** `edge` (ports, refusals, limits, conditional, range, 404, headers), `crates/server/edge/axum`
  (hyper loop, streaming, budgets, events, `/healthz`, graceful stop), `crates/server/edge/folder` and
  `ink serve`. The API, intake and the polling watch are built (0052, 0066, 0077). Open: on-demand
  ACME and TLS, the worker adapter, redirect and variant entries in the manifest.
- `web`: dashboard (Site, What we understood, Deploys, Settings, Account) on the chosen stack,
  pen through wasm, live build log as a streamed page **(owner: dashboard stack, after the frontend
  research)**. **Built (0053):** `web` renders the five screens, the replay of one decision, the
  build log (streamed while it runs) and every failure from typed view models with `unspliced`,
  goldens and structure checks; `ink serve --data` mounts it on the API host through `Screens` (0092). Open:
  the decisions diff, identities, the pen's wasm loader (previews are built, 0091; custom domains are built, 0093 and 0101).
- Sign-in for self-host: passkeys, a first-run admin link printed once, and an email link that
  starts a delayed recovery (never a login), per the scheme in `auth.md` (0068, accepted). **Built
  (0066):** the magic-link bootstrap, sessions and the mail port. Its three spikes come first for
  passkeys: `webauthn-rs-core` on `wasm32-unknown-unknown`, the browser matrix, and the licence
  list. **Built (0068, slice 1):** `ink login`, `logout`, `whoami`, `key`, the `Ink-Sig-1` profile and
  device-code enrolment replace the owner token. **Built (0068, slice 2):** passkeys behind the
  `Passkeys` port (`webauthn-rs-core`, builds for wasm32), the dashboard sign-in and list; step-up
  (0084) and recovery codes (0085) are built; the delayed email recovery and the browser matrix are open.
- `ink push` and `ink:source-upload`; `ink status`, `watch`, `deploys`, `rollback`, `resume`,
  `export`.
- CLI redesign (0061, Accepted; `docs/cli.md`): the typed output grammar and one renderer, then the
  grouped command tree with global `-C`, help, completions and `ink man`, before the verbs above are
  built, so each uses the blocks. **(owner: review the design and its open choices)**

**Exit:** on a fresh machine, `ink serve`, sign in, add a repo, see it live, push a change, see
the update, roll back, export, delete: every step with no flags and every default printed.

## M4: Hosted

The same product on Cloudflare. The implementation plan is `docs/hosted.md` (decision 0079,
proposed): work packages in dependency order, what each owns, the conformance suite it must
pass, what runs in `workerd` with no account, and the owner questions with defaults. Crates are
named for the layout after `docs/layout.md` S6; no M4 crate is added to the workspace before that
freeze.

- Built, the Cloudflare rig, locally with no account (0096 to 0098): `cargo xtask worker <case>` and the stage `worker`
  (farm job `worker`, x86 hosts with node) run Workers in `workerd`. H0b: the Worker with the passkey verifier is 1.11 MiB
  raw and 0.37 MiB gzip, sign-in runs, registration needs a clock patch; D1 carries integers to 2^53 - 1; R2 needs a
  known length, so the gateway hashes while streaming and uploads in 8 MiB parts; the container start needs the account.
  H1: `db-conformance` is a list of async cases, and `crates/port/db/d1` passes all 14 in `workerd`. H1b: the typed
  migrations are rendered to wrangler files and applied with `wrangler d1 migrations apply --local` to the same schema as SQLite.
- Planned adapters, in parallel after H1: `crates/port/blob/r2`, `crates/port/queue/cf` (the rows of `crates/port/queue/rows` over D1 plus a
  wake-up, since Cloudflare Queues has no lease or dedupe key), `crates/port/secret/worker`, `crates/port/mail/cf`,
  `crates/server/watch/worker`, the nonce store as a per-account Durable Object (`auth.md` section 5).
- Planned: `crates/server/edge/worker` (serve, API, dashboard, sign-in) over the `Send` twin of the edge traits (0051);
  the per-handle build Durable Object and `crates/port/build/container`, with the container's blobs written
  through the egress handler (0037); deploy pipeline from CI with `wrangler`.
- `control` features, tested natively on SQLite, in parallel (S2 has landed): handles with reservations
  and tombstones **(owner: O3, Q5)**; budgets enforced and the limits page **(owner: O7, Q13)**;
  deletion with receipt; abuse report endpoint and takedown; orgs, roles and per-account
  recovery policy (`auth.md` sections 8, 9, 12); previews of branches **(owner: Q7)**; custom
  domains through Cloudflare for SaaS (our TXT proof first, then the hostname).
  Built so far, natively on SQLite (the owner's defaults of `docs/hosted.md` section 12): C1, handles with a brand
  list, a twelve month hold and verified handles that are never reclaimed (0087); C2, budgets as one typed
  `Limits` (1 GiB, 60 builds a day, 10 minutes a build, 20 deploys kept) that the refusals and the public `/limits`
  page both read, with a retention sweep (0088); C4, unsigned abuse reports limited per source and operator
  suspension that serves a notice and no bytes (0089); C3, removal in bounded steps with a receipt that
  stays whole across a resume (0090); C6, branch previews at `label--handle` that expire, are swept and
  never touch the deploys (0091), pushed by archive until the watch can build a branch; C7, custom domains as
  rows proved by a TXT token that binds the name to a site, with a `Resolver` port and a fake, served at
  once by the edge and released with the site (0093); C8, the email authority, the magic link spent as a
  proof of an address under its own digest key and with no session (0094); C5, organizations as accounts with members whose roles
  are code, the last owner held by one statement, and deploy keys that outlive their approver (0095). `ink abuse
  list|suspend|reinstate` calls the operator routes of C4 with the operator key (0100). `ink domain
  add|list|prove|remove` and the Settings forms are built, and `ink serve` asks a real nameserver for the proof
  record through a small typed DNS client (`--resolver`, `--resolver-timeout`; 0101). The watch lists the
  branches of a public git site and queues a preview build of each, up to `--preview-branches` (default 10, `0` is
  off), through a `pin` table that keeps the branch and the commit seen (migration 308); `ink preview list|remove`
  and `DELETE /v1/sites/{site}/previews/{label}` read and end them (0102). Organizations are reachable: `ink org
  create|list|member add|remove|role|list`, `--org` on the site verbs, `/v1/orgs` routes that act as the organization
  (0103), a typed name and a roster for org deploy keys (0104), and four forms on the Account screen (0105). Left: a
  leave verb, and an org choice on the dashboard's approve page.
- `ink:authority-github` (GitHub App, webhooks, commit statuses), `ink:authority-email`,
  `ink:authority-dns`.
- Versioned inference rules per 0042: `site.rules`, amendments, pending changes, `ink update`
  (engine work, native, no hosted dependency).
- OIDC trusted publishing for CI after launch (open choice 6).
- **(owner)** The Cloudflare account, the zone, billing, R2 buckets and their jurisdiction, the
  mail provider and the GitHub App are owner steps; `docs/hosted.md` section 12 lists each with a
  recommended default. Nothing outward-facing is done before they are answered.

**Exit:** a GitHub repo goes from sign-in to live at `<handle>.uses.ink` in about a minute; a
push is live within the measured time; a custom domain goes live with a valid certificate.

## M5: Launch

- Landing at `uses.ink` from the doodle mock, on the web crate, pen through wasm.
- Docs site, built by `ink` from `docs/` with a uses.ink theme (dogfood). Built with `theme-quiet` as it is
  (`docs/site.md`, 0120): `cargo xtask check sites` builds it twice and refuses any diagnostic or a second
  deploy that differs. Gaps, each needing engine or theme work and none worked around: a sidebar and previous
  and next links (the theme draws only the pinned header `nav`); theme tokens (T3), so the uses.ink look of
  `crates/core/token/token.toml` cannot reach the theme; an outline of a page; a pin of `page.kind` for a
  folder, since a dated decision is a post and the home lists it under "Posts"; deploying the site once
  `uses.ink` is live.
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
