# Conventions

How everything in uses.ink is named and laid out, and the rules code follows. Rules that bind
first-party code bind third-party plugins too. Where a tool can check a rule, it does: workspace
lints in `Cargo.toml`, `clippy.toml`, `deny.toml`, and `cargo xtask check`. The rest are review
rules. When a rule and a deadline collide, the rule wins or the exception is written next to the
code with its reason.

Lineage: these are the agon and rien conventions, adapted. Decisions live in `decisions.md`; this
page only says how things are named and written.

## Vocabulary

Each word means one thing everywhere: code, CLI, dashboard copy, docs, diagnostics.

| Word | Means | Never used for |
|---|---|---|
| **site** | one published blog | the uses.ink service |
| **handle** | the name that owns `<handle>.uses.ink` | an account, a forge username |
| **account** | a person signed in to uses.ink | a handle |
| **identity** | a proven login at an authority (`github:alice`) | an account |
| **authority** | something that proves control: a forge, DNS, a DID, an email inbox | a source |
| **proof** | a dated, typed record that an identity controls something | a session |
| **source** | where content comes from: a source plugin plus a locator | a repo |
| **ability** | something a source can do: `snapshot`, `history`, `watch`, `private` | a host function |
| **snapshot** | an immutable, hashed tree from a source at one version | a deploy |
| **file** | a path in a snapshot | a page |
| **page** | one routed document on a site | a file |
| **post** | a dated page in a collection | any page |
| **format** | a plugin that reads files into pages (markdown, obsidian, typst) | a theme |
| **theme** | a plugin that renders pages into files | a format |
| **decision** | one recorded inference: code, subject, value, evidence, pin | a setting |
| **pin** | an `ink.toml` entry that fixes a decision | a dashboard setting |
| **build** | one run of the pipeline from a snapshot | a deploy |
| **deploy** | the hashed manifest a build produced | a build |
| **live** | the deploy a site serves right now | |
| **preview** | a deploy that is not live, reachable by its own URL | a draft |
| **diagnostic** | a problem: code, severity, cause, fix | a log line |
| **notice** | an announcement that is not a problem, such as a default being chosen | a diagnostic |
| **event** | one entry in a build's or a site's append-only log | a diagnostic |
| **capability** | one host function a plugin may import (`pen.stroke`) | an ability |
| **plugin** | one installable package that fills one slot | an interface |
| **slot** | a plugin kind: `source`, `authority`, `format`, `theme` | a plugin |
| **mark** | a pen drawing: underline, box, ring, fill, tick | an icon |
| **stroke** | one pen line, before it becomes a mark | a mark |
| **token** | a typed theme setting an author can set in `ink.toml` (`accent`) | a credential |
| **credential** | a secret that grants access (installation token, refresh token) | a token |

A new word gets a row here before it gets used.

## Grammar

Paths are `noun.noun`, singular, lower case. The same path names a thing on every surface.

| Thing | Form | Examples |
|---|---|---|
| Diagnostic code | `noun.noun.reason` | `source.git.auth`, `source.budget.bytes`, `link.target.ambiguous` |
| Decision code | `noun.noun` | `site.home`, `post.date`, `page.title` |
| Event | past tense of the verb that caused it | `build.started`, `deploy.published`, `domain.verified` |
| Command (API) | `noun.verb`, verb from the closed list | `domain.add`, `deploy.set`, `handle.rename` |
| WIT package | `ink:<noun>` | `ink:theme`, `ink:host` |
| Host capability | `<interface>.<function>` | `pen.stroke`, `text.measure`, `blob.read` |
| Plugin id | `<scope>:<slot>-<variant>` | `ink:theme-quiet`, `bob:theme-zine` |

**The decision code is the `ink.toml` path.** Decision `site.home` is pinned by `[site] home = ...`;
`page.title` on `posts/x.md` is pinned by `[page."posts/x.md"] title = ...`. The dashboard, `ink
explain --pin` and the diagnostics all derive the pin from the code. Nothing writes one by hand.

Case by medium: kebab-case in TOML keys, WIT, plugin ids, diagnostic and decision codes, CLI
flags, URLs. snake_case and CamelCase only inside Rust where the language requires them.

**Verbs** are a closed set. Reads never change anything. Every write accepts `--dry-run` on the
CLI and `dry-run: true` on the API.

| Kind | Verb | Meaning |
|---|---|---|
| read | `list` | every member of a noun |
| read | `show` | one member in full |
| read | `check` | validate without changing anything |
| read | `watch` | stream changes |
| read | `explain` | the decisions behind a member |
| write | `add` | bring a new member into existence |
| write | `remove` | take one out |
| write | `set` | change a value or choose a provider |
| write | `rename` | give a member a new name, keeping its identity |
| write | `publish` | make a deploy live |
| write | `pause` / `resume` | stop and restart automatic publishing |
| write | `verify` | ask an authority to confirm a claim |

CLI subcommands are task verbs and may sit outside this list (`ink build`, `ink push`, `ink
serve`, `ink export`); they are listed in `architecture.md` and nowhere else. A new API verb is
added here first.

Config keys name what they set, never their unit; the value carries the unit
(`max-file = "25 MiB"`, `poll = "10 min"`). Unknown keys are diagnostics, never ignored.

## Names

- Dry and behavioral: a name says what a thing does or refuses to do. No puns, no food, no
  metaphors, no garden words.
- Banned anywhere: `util`, `utils`, `common`, `core`, `helpers`, `misc`, `base`, `shared`,
  `manager`, `handler`, `service`, `impl`. They collect unrelated code.
- Abbreviations only from a closed list: `id`, `url`, `tls`, `api`, `cli`, `db`, `wit`, `ui`,
  `dns`, `did`, `og`, `css`, `html`, `svg`, `toml`. A new one is added here first.
- Singular everywhere, directories included (`plugin/`, `migration/`, `fixture/`).
- Do not repeat the module in the item: `deploy::Manifest`, not `deploy::DeployManifest`.
- Methods read as verbs, accessors as nouns, no `get_`. Booleans are predicates (`is_live`).
  Conversions follow std: `as_` borrows, `to_` copies, `into_` consumes.
- Nest, do not flatten: `Capability::Pen(Pen::Stroke)`, not `PenStroke`; `source.git.auth`, not
  `source_git_auth`.

## Crates and directories

The path predicts the name and the name predicts the path.

- A crate is `uses-ink-` plus its path joined with `-`: `plugin/theme/quiet` is
  `uses-ink-theme-quiet` (the `plugin/` segment drops, as in agon), `db/sqlite` is
  `uses-ink-db-sqlite`. The prefix is `uses-ink-`, never `ink-`: crates.io treats `-` and `_` as
  one character, and `ink_*` belongs to Polkadot's ink!.
- The workspace aliases every crate to its short name (`engine = { package = "uses-ink-engine",
  path = "engine" }`), so code says `engine::Site`.
- A crate that does not depend on uses.ink gets no prefix and a name of its own, so it can be
  published and reused as is (the pen, the text measurer). Its name is checked free on crates.io
  when it is cut.
- A port is a directory with the trait crate at its root and one directory per adapter:
  `db/` is `uses-ink-db`, `db/d1/` and `db/sqlite/` are its adapters.
- A plugin lives at `plugin/<slot>/<variant>/`; the slot's WIT lives at `plugin/<slot>/wit/`.
  First-party plugins are laid out exactly like third-party ones.
- `spikes/` holds throwaway experiments. Nothing depends on a spike.
- Repo tasks are `cargo xtask <task>` (`brand`, `wit`, `check`, `fixture`). No loose scripts in
  `scripts/`, no generator files beside the code they generate.
- Binaries: `ink` only. Its crate is `uses-ink-cli` at `cli/`.

## Licence

The service, engine and first-party plugins are AGPL-3.0-or-later. The plugin SDK, the WIT
packages and product-independent crates (the pen, the text measurer) are MIT OR Apache-2.0, so a
third-party theme is not bound by the AGPL. Every crate states its licence through
`[workspace.package]` or its own manifest; `cargo deny` checks dependency licences.

## Code

These are rien's rules, kept whole.

**Generic first.** Arguments take the widest useful bound (`impl AsRef<str>`, `impl
IntoIterator<Item = T>`, `impl AsRef<Path>`). Returns are concrete or `impl Trait`. Dynamic
dispatch only at plugin and port boundaries. Two real callers justify an abstraction; one does
not.

**Std vocabulary before our own.** `From`/`TryFrom` with typed errors, `FromStr`, `Display`
written once on the type it describes, `Default`, derived `Eq`/`Ord`/`Hash`, `Iterator` over
returning a `Vec` that is only iterated.

**No standalone helpers.** Behavior that acts on a type is a method on it; a conversion is a
`From`; a real shared capability with no owner is a small crate named for what it does. No
`utils.rs`, `helpers.rs`, `common.rs`, `misc.rs`.

**No strings where a type belongs.** A path is a validated newtype (`ContentPath`, `Route`), a
kind is an enum, a code is an enum variant with a `const` name. JSON is serde types. HTML is
built through a typed API, never spliced with `format!`. Text shown to a person comes from
`Display` or the diagnostic catalog, written once.

**Errors and events.** Nothing is silent.

- Every error is a typed enum with data in its fields and `source()` for the cause. No `String`
  errors, no `Box<dyn Error>` in library code.
- Denied by lint in library code: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`,
  indexing that can panic on unvalidated input, `println!`, `eprintln!`, `dbg!`.
- No silent fallback. Code that picks a default after a failure, or in the absence of input,
  emits a notice saying so. No `let _ = fallible();`, no `unwrap_or_default()` on input.
- Unknown input is rejected with a typed error, never skipped.

**Determinism.** Engine and plugin code never read the wall clock, the environment, the
filesystem or randomness. Time comes from facts, randomness from seeds. Collections that reach
output are sorted. Floats that reach output go through `libm`, so native and wasm agree.

**Hacks.** Every `#[allow]`/`#[expect]` has `reason = "..."`; prefer `#[expect]`. A workaround is
`// HACK(<condition for removal>): <why>`. No `TODO` without a condition. No commented-out code.
`unsafe` is forbidden at the workspace level.

**One source of truth.** Every fact lives in one place; everything else is generated from it and
a check fails when the copies differ. Dependency versions, lints and package metadata live in the
workspace `Cargo.toml`. WIT is the source of the plugin boundary types. Design tokens live in one
file that generates both CSS custom properties and the pen's palette. Docs point; they do not
copy.

**Size.** One idea per module; a file over about 400 lines or a function over about 40 is asking
to be split. Prefer a table or an enum with methods to a chain of `if`. Public API is small;
`pub(crate)` is the default for sharing. Dependencies flow one way, as in `architecture.md`.

**Types.** Parse, do not validate. Invalid states are unrepresentable. Newtypes for ids, spans,
hashes, sizes, durations. `#[non_exhaustive]` on public enums and structs that will grow.
`Debug` on everything public.

## Tests

- **Fast.** The suite runs in seconds. A test over 100 ms is a bug or `#[ignore = "reason"]`.
  No network, no sleeping, no real clocks. Run with `cargo nextest run --workspace`.
- **Fixtures are files** under the crate's `fixture/`: `name.<ext>` is the input, `name.out`
  the expected result, compared with `insta`. Adding a case is adding a file.
- **Site fixtures** are whole folders (`fixture/site/<name>/`) whose expected output is the
  deploy manifest plus the HTML of every page. `ink build --check` must reproduce the same
  hashes twice and on both native and wasm plugin builds.
- **Ports have conformance suites** that every adapter runs (`db/conformance`,
  `blob/conformance`).
- Tests may `unwrap`.

## Commits

jj, conventional commits, one line, minimal. No AI attribution.
