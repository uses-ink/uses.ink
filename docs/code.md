# How the code is built

`architecture.md` says what the parts are; this page says how the code inside them is shaped. The
rule behind every section: a behaviour is written once, as a generic abstraction, and each case is
a type that plugs into it. Adding a decision, a witness, an output file, a mark or a repo check
is adding a type, never editing a central `match`.

Sketches below are the target. A crate that does not match them yet is being moved onto them
(roadmap, "Code rework").

## 1. Decisions are typed codes

A decision code is a type, not an enum variant. Its value type, its subject and its name travel
together, so a `page.title` decision cannot hold a date.

```rust
pub trait Code: Copy + Ord + 'static {
    const NAME: &'static str;                 // "page.title", also the ink.toml path
    type Value: Value;                        // String, Option<When>, Vec<Tag>, Layout, ..
    type Subject: Subject;                    // Site or File
}

pub struct Decision<C: Code> {
    pub subject: C::Subject,
    pub value: C::Value,
    pub evidence: Evidence,
    pub rejected: Vec<Alternative<C::Value>>,
}
```

`codes!` declares the closed v1 set in one place (`Title => "page.title": String on File`) and
generates the zero-sized code types, the erased `AnyDecision` enum that crosses serde, explain
and WIT, and `Code` impls. It is the one macro the model has: the same impl over a list of
types. Every consumer is generic over `C: Code`; only storage and the wire see the erased form.

`Value` is the trait every decided value implements: `Display` for explain, `Eq` for comparing
alternatives, and the conversions a format (frontmatter) and the config (TOML) need, each a trait
of its own so a value type opts into the surfaces it appears on:

```rust
pub trait Value: Clone + Eq + fmt::Debug + fmt::Display + Serialize {}
pub trait FromFront: Value { const KEYS: &'static [&'static str]; fn read(raw: Raw<'_>) -> Result<Self, Expected>; }
pub trait Toml: Value { const SHAPE: Shape; fn read(item: Spanned<&Item>) -> Result<Self, Shape>; fn write(&self) -> Item; }
```

Generic impls do the repetition once: `Option<T>` reads `false` as none for every `T: Toml`,
`Vec<T>` reads arrays element by element with element spans, `String`, `bool`, `When`, `Slug`,
`Language`, `Author`, `ContentPath` each implement the traits once. A new code needs no reader or
writer of its own.

## 2. Every decision is a ladder of witnesses

All inferrers share one mechanism: an ordered list of witnesses, each able to testify to a value
with its evidence. The first that testifies wins; every later one that also testifies becomes a
rejected alternative with its reason. This is the precedence table of `research/architecture.md`
3.6 as code.

```rust
pub trait Witness<C: Code> {
    fn testify(&self, case: &Case<'_, C>) -> Option<Testimony<C::Value>>;
}

pub struct Ladder<C: Code, W> { witnesses: W, code: PhantomData<C> }   // W: a tuple of witnesses
impl<C: Code, W: Witnesses<C>> Ladder<C, W> {
    pub fn decide(&self, case: &Case<'_, C>) -> Decision<C>;
}
```

`Witnesses<C>` is implemented for tuples of `Witness<C>` (static dispatch, no boxes), and for
`Vec<Box<dyn Witness<C>>>` where a plugin adds witnesses at run time. Witnesses are generic and
reused across codes:

| Witness | Testifies | Used by |
|---|---|---|
| `Pin<C>` | the `ink.toml` value, evidence `Config { span }` | every code |
| `Front<C: FromFront>` | the frontmatter keys `C::KEYS` | title, date, updated, slug, tags, publish, kind, summary, language |
| `Heading` | the first level-1 heading | title |
| `FileName<P>` | a parse of the file name, `P` the parser (date prefix, stem) | date, slug, title |
| `History<F>` | an `Exact` history fact `F` (created, updated); a `NotAfter` fact is a rejection with `OnlyBound` | date, updated |
| `Folder<M>` | a folder marker `M` (`posts/`, `drafts/`, `.obsidian/`) | kind, publish, layout |
| `Derived<C, D>` | a value derived from earlier decisions | site.title from the home page, summary from the body |
| `Fallback<C>` | the default, evidence `Default { reason }`, and a notice | every code |

A decision's ladder is a type alias: `type TitleLadder = Ladder<Title, (Pin<Title>,
Front<Title>, Heading, FileName<Stem>, Fallback<Title>)>`. Reading the precedence of `page.title`
is reading that line.

## 3. Interpret and render are stages

```rust
pub trait Stage<I> {
    type Out;
    fn run(&self, input: I, journal: &mut Journal) -> Self::Out;
}
```

`Journal` is the one sink for decisions, diagnostics and notices: nothing is returned on the
side, nothing is printed. Interpret is a composition of stages (`Root`, `Layout`, `Files`,
`Read<F: Formats>`, `Pages`, `Links`, `Assemble`), each generic over what it needs from the
host (`B: Blobs`, `P: Pins`, `F: Formats`). A stage that cannot continue records why and the
composition stops at the end of that stage, so every problem of a stage is reported.

Render is a list of artifacts, each a type producing served files:

```rust
pub trait Artifact {
    fn files<'s>(&'s self, site: &'s Site, out: &mut Output<'_>);
}
```

`Pages<T: Theme>`, `Feed<S: Syndication>` (with `Atom` and `Rss` as the two syndication
formats), `Sitemap`, `Robots`, `NotFound<T>`, `ThemeFiles<T>`. Headers come from a
`Policy` (`fn headers(&self, media: MediaType, status: Status) -> Headers`), so the CSP lives in
one type. The engine's render is `(Pages<T>, Feed<Atom>, Feed<Rss>, ..)` run in order; adding an
output is adding an artifact to that tuple.

## 4. One walk over a document

```rust
pub trait Visit {
    fn block(&mut self, block: &Block) { walk::block(self, block) }
    fn inline(&mut self, inline: &Inline) { walk::inline(self, inline) }
    fn link(&mut self, target: &LinkTarget, children: &[Inline]) { walk::inlines(self, children) }
    // one method per node kind with a default that recurses
}
pub trait Fold { /* the same, returning a new node */ }
```

Link resolution is a `Fold` (unresolved targets become pages), the summary is a `Visit` that
collects plain text, a theme's HTML is a `Visit` that writes through `unspliced`, outline and
reading time are visits. A new node kind is added to the model once with its default walk, and
every visitor that does not care keeps working.

## 5. Plugins are traits the engine is generic over

```rust
pub trait Format { fn handles(&self, path: &ContentPath) -> bool; fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read; }
pub trait Formats { fn read(&self, path: &ContentPath, bytes: &[u8]) -> Option<Read>; }   // a set of formats
impl<A: Format, B: Formats> Formats for (A, B) { .. }                                      // first that handles it
pub trait Theme { fn render(&self, page: &Page, site: &Site) -> Result<Html, Diagnostic>; fn files(&self) -> Files; }
pub trait Blobs { fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing>; }
```

The engine is `Engine<B, P, F, T>`; the CLI picks native types, the wasmtime host picks
component adapters implementing the same traits, tests pick in-memory ones. Nothing in the engine
names a concrete plugin.

## 6. The pen: tools and marks are types

```rust
pub trait Tool { fn ink(&self, traced: &[Pressured]) -> Path; }            // Pencil, Highlighter
pub trait Mark { fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths; }
```

`Line`, `Underline`, `Frame`, `Ring`, `Fill`, `Patch` and `Drawn` are data (their geometry), each
implementing `Mark`; seeds name a mark (`Seed::of(name).and(i)`), not a function argument
threaded through helpers. `freehand-web` exports one function per mark type, all through one
generic `draw::<M>`. Brand drawings and theme marks are the same `Mark` values.

## 7. Repo tasks are generators, steps and rules

```rust
pub trait Generator { const SOURCE: &'static str; fn generate(&self, source: &str, out: &mut Output) -> Result<(), Error>; }
pub trait Step { fn run(&self, cargo: &Cargo) -> Result<(), StepFailure>; }   // fmt, clippy, nextest, ..
pub trait Rule { fn check(&self, workspace: &Workspace, problems: &mut Problems); }
```

`token`, `diag` and `brand` are generators; `cargo xtask check` runs every step and every rule
(`Names`, `Order`, `Licence`, `Alias`, `Banned`, `Unsafe`, `Words`) and reports all of them. A
new check is a new `Rule`.

## 8. Generic, and where it stops

- Arguments take traits (`impl AsRef<str>`, `impl IntoIterator`, `W: Witnesses<C>`), not
  concrete collections.
- Static dispatch by default (generics, tuples of types); `dyn` only where the set is chosen at
  run time (plugins loaded from config, witnesses a plugin adds).
- A trait needs two real implementations or a plugin boundary. Generic code with one
  instantiation and no boundary is a concrete type instead.
- Generic machinery is tested once with a stand-in, and each case is tested as data: a ladder
  fixture lists inputs and the expected decision with its evidence and rejected alternatives.
