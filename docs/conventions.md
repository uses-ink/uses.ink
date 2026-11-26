# Conventions

How everything in uses.ink is named and laid out, and the rules code follows. Rules that bind
first-party code bind third-party plugins too. Where a tool can check a rule, it does: workspace
lints in `Cargo.toml`, `clippy.toml`, `deny.toml`, and `cargo xtask check`. The rest are review
rules. When a rule and a deadline collide, the rule wins or the exception is written next to the
code with its reason.

Lineage: these are the agon and rien conventions, adapted. How the code is shaped (the shared
generic abstractions every crate plugs into) is `code.md`. Decisions live in `decisions.md`; this
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
| **problem** | what a diagnostic says: its code and typed fields (`Problem`) | a diagnostic |
| **announcement** | what a notice says: its code and typed fields (`Announcement`) | a notice |
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
  `dns`, `did`, `og`, `css`, `html`, `svg`, `toml`, `rgb`, `hex`. A new one is added here first.
  Ported code keeps its original's names (`prev` in `freehand`'s perfect-freehand port) so it
  reads against the source.
- Singular everywhere, directories included (`plugin/`, `migration/`, `fixture/`).
- Do not repeat the module in the item: `deploy::Manifest`, not `deploy::DeployManifest`.
- Methods read as verbs, accessors as nouns, no `get_`. Booleans are predicates (`is_live`).
  Conversions follow std: `as_` borrows, `to_` copies, `into_` consumes.
- Nest, do not flatten: `Capability::Pen(Pen::Stroke)`, not `PenStroke`; `source.git.auth`, not
  `source_git_auth`. Two items sharing a prefix and a reason go in a module named for the prefix,
  and lose it. A wire format nests the same way: `{ "capability": "pen", "call": "stroke" }`.
- Names say what a thing is, not how it is made: `Snapshot`, not `SnapshotBuilderImpl`. No type
  the type system already states (`paths`, not `path_vec`). No `Impl`, `Base`, `Abstract`.
- One word when one word is accurate; a longer name means the thing does two jobs. Short is not
  cryptic: `n` in a three-line scope is fine, anything that lives longer gets a word.
- No boolean parameters: a call reading `draw(x, true)` is an enum (`Cap::Round`). A function
  takes at most four parameters; past that, a struct of options.

## Crates and directories

The path predicts the name and the name predicts the path.

- A crate is `uses-ink-` plus its path joined with `-`: `plugin/theme/quiet` is
  `uses-ink-theme-quiet` (the `plugin/` segment drops, as in agon), `db/sqlite` is
  `uses-ink-db-sqlite`. The prefix is `uses-ink-`, never `ink-`: crates.io treats `-` and `_` as
  one character, and `ink_*` belongs to Polkadot's ink!.
- The workspace aliases every crate to its short name (`engine = { package = "uses-ink-engine",
  path = "engine" }`), so code says `engine::Site`.
- A crate useful outside uses.ink (the pen, the text measurer) gets no prefix and a name of its own,
  so it can be published and reused as is. Its name is checked free on
  crates.io when it is cut.
- A port is a directory with the trait crate at its root and one directory per adapter:
  `db/` is `uses-ink-db`, `db/d1/` and `db/sqlite/` are its adapters.
- A plugin lives at `plugin/<slot>/<variant>/`; the slot's WIT lives at `plugin/<slot>/wit/`.
  First-party plugins are laid out exactly like third-party ones.
- `spikes/` holds throwaway experiments. Nothing depends on a spike.
- Repo tasks are `cargo xtask <task>` (`token`, `diag`, `brand`, `layout`, `check`; `wit` when
  it exists). No loose scripts in `scripts/`, no generator files beside the code they generate.
- Binaries: `ink` only. Its crate is `uses-ink-cli` at `cli/`.
- Cut a crate when its second user appears, not before; its tests move with it. A crate useful
  outside uses.ink is cut with a name of its own (above).
- A third-party crate is wrapped in one module of one crate (gix in `host/native`, comrak in the
  markdown format, wasmtime in `host/wasmtime`); the rest of the code sees our types. Swapping it
  touches one place.
- A new dependency needs its row in `libraries.md` in the same change, saying why; a maintained
  small crate beats writing our own, a large one for a small job does not.
- A directory holds about eight entries at most (tests and support files aside); past that, nest
  by domain.

## Licence

The service, engine and first-party plugins are AGPL-3.0-or-later. The plugin SDK, the WIT
packages and product-independent crates (the pen, the text measurer) are MIT OR Apache-2.0, so a
third-party theme is not bound by the AGPL. Every crate states its licence through
`[workspace.package]` or its own manifest; `cargo deny` checks dependency licences.

## Code

Adapted from rien (structure, strings, one source of truth) and agon (errors at the edge, the
std trait table).

**Generic first.** Arguments take the widest useful bound (`impl AsRef<str>`, `impl
IntoIterator<Item = T>`, `impl AsRef<Path>`). Returns are concrete or `impl Trait`. Dynamic
dispatch only at plugin and port boundaries. Two real callers justify an abstraction; one does
not.

**Std traits first.** A type works with `?`, `.parse()`, `.collect()` and `.into()` because it
implements the standard traits, not a method of its own:

| To do this | Implement | Not |
|---|---|---|
| parse text | `FromStr` (and `TryFrom<String>` when it owns) | `parse`, `from_str`, `from_text` |
| convert, cannot fail | `From<T>` | `from_x`, `wrap`, `new_from` |
| convert, can fail | `TryFrom<T>` with a typed error | a method returning `Option` |
| view as bytes, text or a path | `AsRef<[u8]>`, `AsRef<str>`, `AsRef<Path>` | `as_bytes`, `to_slice` |
| build from many | `FromIterator`, `Extend` | `of_all`, `from_parts` |
| default | `Default` | `empty`, `none`, `standard` |
| show to a person | `Display` | `to_text`, `describe`, `message()` |

A named constructor stays when it says what a trait cannot (`Hash::of(bytes)` hashes); a
conversion that would be a trap gets a named method and its doc says why.

**No free functions.** Behaviour lives on the type it acts on. A free function is allowed only
as a crate's entry point (`markdown::read`, `engine::interpret`, `main`) or a test. Everything
else is one of:

- a method or associated function on the type it reads or builds (`Route::of(page)`, not
  `route_of_page(page)`; `Entry::atom(&self)` on a feed entry, not `atom_entry(entry)`);
- a std trait impl: a conversion is `From`/`TryFrom`, parsing is `FromStr`, text is `Display`
  (frontmatter values are `TryFrom<Raw>` impls, not `text()`, `when()`, `slug()` helpers);
- a method on a small private type made for the job when no type exists yet: a walk over a tree
  is a `Walk` struct with methods, an inferrer is a type implementing the inferrer trait, not a
  module with a `decide` function;
- a trait when several types share the behaviour.

No `utils.rs`, `helpers.rs`, `common.rs`, `misc.rs`. A name of three words or more
(`push_text_node`, `atom_entry_of`) is a missing type or module: `text::Node::push`,
`atom::Entry`. Function and method names are one or two words; `cargo xtask check` refuses
longer ones outside tests (a test's name is a sentence and may be long), and refuses a free
function outside a crate's entry points (the `pub` functions and `main` of a target's root, a
function exported by symbol name, and xtask's `Free::ALLOWED`, kept tiny). Both rules skip
`#[test]` functions, `#[cfg(test)]` modules, test targets and generated files, and count a
trait impl's methods under the trait. They fail only the crates in xtask's `ENFORCED` and count
the others in a note; a crate joins the list when its code passes.

**No strings where a type belongs.** A path is a validated newtype (`ContentPath`, `Route`), a
kind is an enum, a code is an enum variant with a `const` name. JSON is serde types. HTML, SVG
and CSS are built through `unspliced`, never spliced with `format!`. A name built by a rule (a
seed `{name}{i}`, a route, a file name) is built by one method on one type, never by `format!`
at each call site.

**Text is written once.** Text shown to a person comes from `Display` on the type it describes,
or from the diagnostic catalog. `format!` is for a one-off line with a few holes; once a line is
built the same way twice, it is a `Display` impl. A report line (`wrote brand/logo.svg`) is an
enum with `Display`, not a string pushed into a `Vec<String>`. Nothing parses a message.

**Errors.** Nothing is silent.

- Each crate owns its error enum: one variant per failure, the data in typed fields (paths,
  names, spans, counts, codes), the cause in a `#[source]`-style field returned by `source()`.
  No `String` and no `&'static str` reason fields: a reason with a rule is a variant or an enum.
- A cause is kept, never flattened: no `.to_string()`, `format!("{e}")` or
  `String::from_utf8_lossy(stderr)` into an error field. A child process's failure keeps its
  exit status and its stderr as bytes, in a type of its own.
- No aggregate error type and no `Box<dyn Error>` in library code. Erasure to text happens once,
  at the edge (the CLI or the API), through `Display` and the `source()` chain.
- Problems an author or operator sees go through the catalog (`diag/catalog.toml`), not through
  a crate error: the crate error converts into a `Diagnostic` with a code at the boundary.
- Denied by lint in library code: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`,
  indexing that can panic on unvalidated input, `println!`, `eprintln!`, `dbg!`.
- No silent fallback. Code that picks a default after a failure, or in the absence of input,
  emits a notice saying so. No `let _ = fallible();`, no `unwrap_or_default()` on input. Writing
  into a `String` is infallible and is the one `let _infallible = write!(..)` allowed, named so.
- Unknown input is rejected with a typed error, never skipped.

**Look before you write.** Search for the concept first; a second copy of anything is a missing
type or method. A special case for one caller lives in that caller's module, never as a flag on a
shared one.

**Determinism.** Engine and plugin code never read the wall clock, the environment, the
filesystem or randomness. Time comes from facts, randomness from seeds. Collections that reach
output are sorted. Floats that reach output go through `libm`, so native and wasm agree. Anything
drawn from a seed consumes its draws in a fixed order: reordering draws redraws every mark, so a
change there diffs the goldens on purpose.

**Macros.** `macro_rules!` only where a function or a generic cannot remove the repetition (the
same impl over a list of types, test tables). Never to hide control flow or invent syntax. Each
has a doc comment with an example and expands clean under clippy. A proc macro needs a written
reason.

**Features** turn optional capabilities on and never change what existing code does: additive,
off by default when they pull a dependency, named for the capability (`serde`, `schema`), with
`dep:` for optional dependencies, documented above the feature in `Cargo.toml` and in the crate
docs. Gate the narrowest item (`cfg_attr` on a derive, one `impl`), never `cfg!` inside a body.
CI builds each crate with no features, each alone, and all.

**Privacy.** Nothing is logged that holds a credential, a token or a person's content; a type
holding a secret redacts itself in `Debug` and has no `Display`. No telemetry.

**Hacks.** Every `#[allow]`/`#[expect]` has `reason = "..."`; prefer `#[expect]`. A workaround is
`// HACK(<condition for removal>): <why>`. No `TODO` without a condition. No commented-out code.
`unsafe` is denied at the workspace level; `cargo xtask layout` allows it only in the files
xtask's `Unsafe::ALLOWED` lists, each because a decision says.

**One source of truth.** Every fact lives in one place; everything else is generated from it and
a check fails when the copies differ. Validate once, in a type: code holding a `Slug` does not
check it again. Dependency versions, lints and package metadata live in the workspace
`Cargo.toml`; a version in a crate manifest is a bug. WIT is the source of the plugin boundary
types. Design tokens live in one file. Docs point; they do not copy.

**Size.** One idea per module; a file over about 400 lines or a function over about 40 is asking
to be split. Prefer a table or an enum with methods to a chain of `if`. Public API is small;
`pub(crate)` is the default for sharing. Dependencies flow one way, as in `architecture.md`.

**Types.** Parse, do not validate. Invalid states are unrepresentable: two bools that exclude
each other are an enum. Newtypes for ids, spans, hashes, sizes, durations. `#[non_exhaustive]`
on public enums and structs that will grow. `Debug` on everything public; every public item has
a doc comment.

## Tests

- **Fast.** The suite runs in seconds. A test over 100 ms is a bug or `#[ignore = "reason"]`.
  No network, no sleeping, no real clocks. Run with `cargo nextest run --workspace`.
- **Fixtures are files** under the crate's `fixture/`: `name.<ext>` is the input, `name.snap`
  the expected result. One harness runs them all: `uses-ink-fixture` (`fixture/`), a
  dev-dependency any crate may take. `fixture::files(dir, ext, render)` runs each input through
  `render`; `fixture::cases(dir, outputs)` takes outputs built in code. It compares through
  `insta` and updates with `INSTA_UPDATE`, never a bless variable per crate. Adding a case is
  adding a file.
- **Goldens are read, never trusted.** An update (`INSTA_UPDATE=always` or `=new`) writes the
  new goldens and the run fails listing them, so it cannot pass unnoticed; read every changed
  golden before committing.
- **Harness failures are loud.** An empty suite, an input without a golden, a golden without an
  input and a `.snap.new` left beside them all fail.
- **Site fixtures** are whole folders (`fixture/site/<name>/`) whose expected output is the
  deploy manifest plus the HTML of every page. `ink build --check` must reproduce the same
  hashes twice and on both native and wasm plugin builds.
- **Ports have conformance suites** that every adapter runs (`db/conformance`,
  `blob/conformance`).
- **Refusals are visible.** A test of a refusal checks the diagnostic or notice it emits, not only
  the returned error.
- Properties beat examples for parsers and the model; a property test is seeded and prints its
  seed. A bug fix starts with a failing fixture or test.
- **Hostile input gets tests**: malformed files, huge and non-finite numbers, and for plugins one
  that spins, allocates without end, overflows its stack or returns garbage, with a check that
  the host serves afterwards. Budgets in tests are tight (a spinning plugin is cut at 50 ms).
- **Plugins have a native core.** A plugin keeps its rule in plain Rust, tested natively in
  milliseconds; its wasm glue is thin, and one differential test per plugin calls the real
  component and compares it with the native rule. Components are built once per run, not per
  test.
- Expensive setup is done once and cached under `target/`, since nextest runs each test in its
  own process. Dependencies build optimized in the dev profile; our crates keep debug assertions.
- Test behaviour through the public API; a crate does not expose internals to make a test easier.
- Tests may `unwrap`.

## CLI output

- Results go to stdout; everything else (progress, notices, diagnostics) to stderr. `--json`
  puts the result on stdout as JSON and streams NDJSON events with a `schema` field.
- A diagnostic is one block: `error[code]: wording` from the catalog, then each context field
  once as `key: value`, the span with its source line where there is one, then `fix:`, then the
  `caused by` chain. A notice is one line: `note[code]: wording`.
- One-shot commands log at warn with no timestamps; `ink serve` logs at info with timestamps.
  `-v`/`-q` move one level and win over `INK_LOG`, which wins over `RUST_LOG`.
- Exit codes are a closed, documented set (`research/flow.md` section 7).

## Docs and prose

- Every public item has a doc comment saying what and why, not how; a crate starts with a `//!`
  line saying what it is (`missing_docs` warns).
- A decision lives in `decisions.md`; `architecture.md` is changed in the same commit as code that
  makes it wrong. Docs and comments link; they do not restate.
- Plain sentences, concrete nouns. No em dashes, no unicode arrows in prose, no marketing words.

## Commits and checks

- jj, conventional commits, one line, minimal, imperative, lower case after the colon. No AI
  attribution.
- One logical change per commit: a refactor and a behaviour change are two commits.
- Every commit passes the checks: `cargo xtask check` runs all of them in CI's order (fmt, clippy
  natively and on wasm32, nextest, doc tests, `cargo deny`, the pen's tests on wasm32-wasip2, the
  browser pen's release build, then layout and generated files), every one with `--locked`. It
  runs every step and lists each failure; a missing tool fails with the command that installs it.
  CI runs that one command, so the list lives in one place (`Gate::steps` in `xtask/src/check.rs`).
  `cargo xtask layout` runs the last step alone, for a fast loop.
- After each milestone, read-only review agents go over its diff; every finding that holds is
  fixed, and the ones skipped are written down with the reason.
