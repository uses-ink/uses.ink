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
pub trait Code: Copy + Ord + Hash + Default + fmt::Debug + Send + Sync + 'static {
    const NAME: &'static str;                 // "page.title", also the ink.toml path
    const CODE: AnyCode;                      // the code, erased
    type Value: Value;                        // String, Option<When>, Vec<Tag>, Layout, ..
    type Subject: Subject;                    // decision::Site or decision::File
    fn erase(decision: Decision<Self>) -> AnyDecision;                       // written by codes!
    fn claim(decision: AnyDecision) -> Result<Decision<Self>, AnyDecision>;
    fn view(decision: &AnyDecision) -> Option<&Decision<Self>>;
}

pub struct Decision<C: Code> {
    pub subject: C::Subject,
    pub value: C::Value,
    pub evidence: Evidence,
    pub rejected: Vec<Alternative<C::Value>>,
}
```

A subject kind is a type too: `decision::Site` and `decision::File(ContentPath)` implement
`decision::Subject` (with `const SCOPE: Scope`) and convert to and from the erased
`model::Subject` that diagnostics, storage and the wire use, refusing the other scope with a
typed `Mismatch`.

`codes!` declares the closed v1 set in one place (`Title => "page.title" on File: String, keys
["title"];`) and generates the zero-sized code types (`model::code::Title`), their `Code` impls,
`Keyed` impls for codes with frontmatter keys, the erased `AnyCode` (name, scope, `ALL` in
decision order) and the erased `AnyDecision` enum that crosses serde (tagged by the code's
name), explain and WIT. It is the one macro the model has: the same impl over a list of types.
Every consumer is generic over `C: Code`; only storage and the wire see the erased form, through
`From<Decision<C>> for AnyDecision`, `TryFrom<AnyDecision> for Decision<C>` and
`AnyDecision::get::<C>()`. Work on any code is written once, generically: `AnyCode::each` runs an
`Each<C>` for the code a name picks, and `AnyDecision::each` runs a `Typed<C>` on a decision as
its code's type (config pins every decision this way); either compiles only when every code
meets the impl's bounds.

`Value` is the trait every decided value implements: `Eq` for comparing alternatives, serde for
storage, and `show` for explain (not `Display`: `Option<T>` and `Vec<T>` are values and cannot
implement it; `Shown(&value)` is the `Display`). Each surface a value appears on is a trait of
its own, so a value type opts into it:

```rust
pub trait Value: Clone + Eq + fmt::Debug + Serialize + DeserializeOwned { fn show(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result; }
pub trait FromFront: Value { const WANTED: Wanted; fn read(raw: &Raw<'_>) -> Result<Self, Refusal>; fn many(raw: &Raw<'_>) -> Result<Vec<Self>, Refusal>; }
pub trait Keyed: Code<Subject = File, Value: FromFront> { const KEYS: &'static [&'static str]; }
pub trait Toml: Value { const SHAPE: Shape; fn read(item: Spanned<&Item>) -> Result<Self, Shape>; fn write(&self) -> Item; }   // in config
```

Frontmatter keys belong to the code, not the value type: `String` is a title under `title` and a
summary under `description` or `summary`, so `KEYS` is on `Keyed`. A format parses its syntax
into `Raw` (null, a boolean or a number with its text as written, text, a list, a mapping, each
with its span) and the value type reads that; a refusal is an `Expected` (what was wanted, one
or a list, the span, the value type's own cause), and a `Refusal` holds every refused item of a
list. `Toml` lives in the config crate, which owns `toml`; the model defines nothing for it.

Generic impls do the repetition once: `Option<T>` reads null and `false` as none, `Vec<T>` reads
null as empty and a list item by item, refusing the whole value with every bad item's span (a
scalar is a list of one, except `Tag`, which also splits text at commas). `String`, `bool`,
`When`, `Slug`, `Language`, `Author`, `Tag`, `PageKind`, `ContentPath` each implement the traits
once. A new code needs no reader or writer of its own.

## 2. Every decision is a ladder of witnesses

All inferrers share one mechanism: an ordered list of witnesses, each able to testify to a value
with its evidence. The first value testified wins; every later one becomes a rejected
alternative, outranked; a bound is always rejected as one. This is the precedence table of
`research/architecture.md` 3.6 as code.

```rust
pub trait Witness<C: Code, G: ?Sized> {
    fn testify(&self, case: &Case<'_, C, G>, testimony: &mut Testimony<C::Value>);
}
pub struct Case<'a, C: Code, G: ?Sized> { subject: &'a C::Subject, pin: Option<Pin<C::Value>>, patterns: &'a dyn Patterns, ground: &'a G }

pub struct Ladder<C, W> { witnesses: W, code: PhantomData<C> }   // W: a tuple of witnesses
impl<C: Code, W> Ladder<C, W> {
    pub fn decide<G: ?Sized>(&self, case: &Case<'_, C, G>) -> Result<Decision<C>, Error> where W: Witness<C, G>;
}
```

`G` is the ground the deciding stage holds: one file, the snapshot, the site so far. A witness
is generic over the grounds it can read through two small traits, `Filed` (a file's paths and
facts) and `Sourced` (a page a format read: the file itself, or the site's home), so `Front<C>`
reads a page's frontmatter on a file's ground and the home's on the site's. A witness adds to the
`Testimony` (a value, a bound, or a failure), so one witness can testify several times (`Index`
for each home candidate) and a later one can see what was said (`Fallback`, `Otherwise`).

A tuple of witnesses is itself a witness (static dispatch, no boxes), and so is
`Vec<Box<dyn Witness<C, G>>>`, where a plugin adds witnesses at run time. Witnesses are generic
and reused across codes; what a witness means for a code is a trait the code implements:

| Witness | Testifies | Code trait |
|---|---|---|
| `Pin<C>` | the `ink.toml` value, evidence `Config { span }` | |
| `Front<C>` | the frontmatter field of `C` | `Fronted` |
| `Draft` | `draft: true`, the opposite of `publish` | |
| `Heading` | the first level-1 heading | |
| `Body` | the first paragraph, cut near a sentence end (a `Visit`) | |
| `FileName<P>` | a parse of the file name, `P` the parser (`Dated`, `Stem`, `Repository`) | `Parse<C>` on `P` |
| `History<F>` | a history fact `F` (`Created`, `Changed`): a value when exact, a bound when `NotAfter` | |
| `Folder<M>` | a folder marker `M` (`Posts`, `Drafts`, `Hidden`, `Obsidian`, `Docs`, `Content`) | `Mark<C>` on `M` |
| `Pattern<L>` | an `include` or `exclude` pattern | `Listed<L>` |
| `Derived<D>` | the home page's decision of code `D` | `Follows<D>` |
| `Implied<D, W>` | what witness `W` says of code `D` on the same subject | `Follows<D>` |
| `Otherwise<W>` | what `W` says, only when nothing before it said a value | |
| `Fallback<C>` | the default, evidence `Default { reason }` | `Assumed<G>` |

A decision's precedence is a type alias, one per code, in `engine::interpret::decide::precedence`:

```rust
type Title = Ladder<code::Title, (Pin<code::Title>, Front<code::Title>, Heading, Otherwise<FileName<Stem>>)>;
```

Reading the precedence of `page.title` is reading that line. What a code's values mean to a
person is the code's own, not its value type's: `model::Word` gives each code a `NONE` phrase
(`undated`, `never updated`, `a generated listing`) and a `word` for its values (`published`,
`ignored`), and `Worded::<C>` is the `Display`. `ink explain`, the engine's notices and
`ink.toml`'s errors (`a date, or false for undated`) all word a value through it. Which of a
code's defaults is announced is the engine's `Explain`, a subtrait of `Word`.

## 3. Interpret and render are stages

```rust
pub trait Stage<I> {
    type Out;
    fn run(&self, input: I, journal: &mut Journal) -> Self::Out;
}
```

`Journal` is the one sink for decisions (erased as `AnyDecision`, read back typed), diagnostics
and notices: nothing is returned on the side, nothing is printed. Interpret is a composition of
stages (`Root`, `Layout`, `Files`, `Read<B, F>`, `Pages`, `Links`, `Assets`, `Site`, `Routing`,
`Assemble`), each generic over what it needs from the host (`B: Blobs`, `P: Pins`,
`F: Formats`). Every stage runs, so every problem of every stage is reported; `Assemble` builds
no site from a journal holding an error.

Render is a tuple of artifacts, each a type producing served files:

```rust
pub trait Artifact {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error>;
}
```

`Pages<T>` (and a generated home), `NotFound<T>`, `Assets`, `Feed<S: Syndication>`, `Sitemap`,
`Robots`, `ThemeFiles<T>`. A tuple of syndication formats is one, so `Feed<(Atom, Rss)>` gathers
the posts and says what it leaves out once. Headers come from one `Policy`
(`fn headers(&self, media: MediaType) -> Vec<Header>`), so the CSP lives in one type; quick-xml
is seen only by `render::xml`. The engine's render is
`(Pages<T>, NotFound<T>, Assets, Feed<(Atom, Rss)>, Sitemap, Robots, ThemeFiles<T>)` run in
order; adding an output is adding an artifact to that tuple. An artifact that fails is recorded
and the next still runs.

## 4. One walk over a document

```rust
pub trait Visit {
    fn block(&mut self, block: &Block) { block.walk(self) }
    fn inline(&mut self, inline: &Inline) { inline.walk(self) }
    fn link(&mut self, target: &LinkTarget, children: &[Inline]) { self.inlines(children) }
    // document, footnote, blocks, item, definition, inlines, image, text: each defaults to
    // the node's own walk
}
pub trait Fold { /* the same methods, taking the node and returning its replacement */ }
impl Block { pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V); pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self; }
```

The default recursion lives on the nodes (`Document`, `Footnote`, `Block`, `Item`, `Definition`,
`Inline` each have `walk` and `fold`), not in a module of free functions: a visitor that
overrides a method calls `node.walk(self)` to keep descending.

Link resolution is a `Fold` (unresolved targets become pages), the summary is a `Visit` that
collects plain text, a theme's HTML is a `Visit` that writes through `unspliced`, outline and
reading time are visits. A new node kind is added to the model once with its default walk, and
every visitor that does not care keeps working.

## 5. Plugins are traits the engine is generic over

```rust
pub trait Format { fn handles(&self, path: &ContentPath) -> bool; fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read; }
pub trait Formats { fn handles(&self, path: &ContentPath) -> bool; fn read(&self, path: &ContentPath, bytes: &[u8]) -> Option<Read>; }
impl<A: Format, B: Format> Formats for (A, B) { .. }   // tuples up to four: the first that handles it
pub trait Theme { fn name(&self) -> &str; fn assets(&self) -> Vec<File>; fn render(&self, page: &Page, site: &Site) -> Result<Html, Diagnostic>; }
pub trait Blobs { fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing>; }
pub trait Pins: Patterns { fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>>; }
```

The engine is `Engine<B, P, F, T>`, with `interpret` needing the first three and `render` the
theme, so a part a call does not use is `()`; the CLI picks native types, the wasmtime host picks
component adapters implementing the same traits, tests pick in-memory ones. Nothing in the engine
names a concrete plugin.

## 6. The pen: tools and marks are types

```rust
pub trait Chance { fn draw(&mut self) -> f64; fn fork(&self, part: impl Display) -> Self; } // Mulberry, Constant
pub trait Tool { fn ink<C: Chance>(&self, line: &Centreline, hand: &Hand, chance: &mut C) -> Path; } // Pencil, Highlighter
pub trait Mark { fn draw<T: Tool, C: Chance>(&self, hand: &Hand, tool: &T, chance: &mut C) -> Paths; }
```

The marks in `freehand::mark` (`Line`, `Underline`, `Frame`, `Ring`, `Fill`, `Patch<P>`, `Drawn<L>`)
are data: their geometry, generic over how it is held (`Patch` takes any `AsRef<[Point]>`,
`Drawn` any `Borrow<Centreline>`). Each implements `Mark` for any tool, hand and chance; a line's
weight is its pencil's. A seed names a mark: the caller draws from
`Mulberry::from(Seed::of(name))`, and a mark of several lines draws line `i` from `chance.fork(i)`,
the seed `Seed::of(name).and(i)`, not from a name threaded through helpers. The perfect-freehand
port keeps its loops and numbers; what were its free functions are methods on what they act on
(`Taper::length`, a `Sweep` iterator for caps, `Dot`, `Sides::finish`, `Tenths::nearest`,
`Centreline::normal`). `freehand-web` describes each export as a `Call` (tool, hand, input
lengths) drawn through one generic `Call::draw::<M: Mark>` into its `Buffer`. Brand drawings go
through the same tools and seeds.

## 7. Repo tasks are generators, steps and rules

```rust
pub trait Generator {
    const TASK: Task;                                              // named in each file's header
    const SOURCE: &'static str;                                    // "token/token.toml"
    type Source: TryFrom<toml::Table, Error: Into<Flaw>>;          // Tokens, Catalog, Figure
    fn generate(&self, source: &Self::Source, files: &mut Files<'_>) -> Result<(), Error>;
}
pub trait Step: Display { fn run(&self, root: &Path, report: &mut Report); }
pub trait Rule { const NAME: &'static str; fn check(&self, workspace: &Workspace, problems: &mut Problems); }
```

`Token`, `Diag` and `Brand` are generators. `Output::generate::<G>` reads `G::SOURCE`, parses it
into the typed `G::Source` (a source that does not parse is an `Error::Source` naming the file),
and hands the generator a `Files` that writes or, for `layout`, compares each file and records
which task a stale one comes from. The generators' Rust is built through `ToTokens` impls
(`Palette`, `Faces`) and methods on the catalog's types, never free functions.

The gate is a tuple of steps (`Gate::steps`): each cargo step is one generic
`Cargo<S: Scope>` (a `Command`, a scope of `Every` crate, a `[&str; N]` of packages or `()`,
typed `Flag`s and a `Target`), whose command line and needed `Tool`s follow from that data, and
the last step is `Layout`. Rules run over a `Workspace` read once: `cargo metadata`, the
`[workspace.dependencies]` aliases, every path in the tree, every `.rs` file per crate, and the
module tree each target compiles, parsed with `syn` and marked entry, inner, test or generated.
The rules are `Names`, `Order`, `Licence`, `Alias`, `Banned`, `Unsafe` (on tokens, so comments
and literals never count), `Words` and `Free`. `Gradual<R>` wraps a rule so it fails only the
crates in `ENFORCED` and counts the rest in a note, which is how `Words` and `Free` land while
the crates move onto them. Lists of generators, steps and rules are tuples, given their list
trait by one `tuples!` macro. `cargo xtask check` runs every step and reports every failure; a
new check is a new `Rule` added to `Layout::RULES`.

## 8. Generic, and where it stops

- Arguments take traits (`impl AsRef<str>`, `impl IntoIterator`, `W: Witnesses<C>`), not
  concrete collections.
- Static dispatch by default (generics, tuples of types); `dyn` only where the set is chosen at
  run time (plugins loaded from config, witnesses a plugin adds).
- A trait needs two real implementations or a plugin boundary. Generic code with one
  instantiation and no boundary is a concrete type instead.
- Generic machinery is tested once with a stand-in, and each case is tested as data: a ladder
  fixture lists inputs and the expected decision with its evidence and rejected alternatives.
