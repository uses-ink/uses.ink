# How the code is built

`architecture.md` says what the parts are; this page says how the code inside them is shaped. The
rule behind every section: a behaviour is written once, as a generic abstraction, and each case is
a type that plugs into it. Adding a decision, a witness, an output file, a mark or a repo check
is adding a type, never editing a central `match`.

Sketches below are the code as built.

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
    pub evidence: Evidence,      // the winner's alone (0042); `evidence.basis()` is Stated, Inferred or Assumed
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

`Value` is the trait every decided value implements: `Eq` for comparing two builds' values, serde for
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
with its evidence. The first value testified wins and is recorded with its evidence alone; a
bound is never chosen (0042). This is the precedence table of `research/architecture.md` 3.6 as
code.

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

A tuple of witnesses is itself a witness (static dispatch, no boxes); each member is `Named`
(`const NAME: Witnessed`, worded by `Display`: "front matter", "first heading", "posts
folder"), and the tuple starts a rung of the `Testimony` before each one. A normal run keeps only
the decision. When `ink explain --why` asks for one decision, `Engine::trace` interprets with a
`Journal` that names it, and that ladder alone runs with tracing on: the `Trace` holds every
rung, what it said (as the decision it would have been, a value or a bound) or nothing, and which
testimony was chosen. Nothing about a trace reaches the model, WIT or the record. Witnesses a
plugin adds at run time will be a named list of their own when the first plugin needs one.
Witnesses are generic and reused across codes; what a witness means for a code is a trait the
code implements:

| Witness | Testifies | Code trait |
|---|---|---|
| `Pin<C>` | the `ink.toml` value, evidence `Config { span }` | |
| `Front<C>` | the frontmatter field of `C` | `Fronted` |
| `Draft` | `draft: true` as not published; `draft: false` says nothing | |
| `Heading` | the first level-1 heading | |
| `Body` | the first paragraph, cut near a sentence end (a `Visit`) | |
| `FileName<P>` | a parse of the file name, `P` the parser (`Dated`, `Stem`, `Repository`) | `Parse<C>` on `P` |
| `History<F>` | a history fact `F` (`Created`, `Changed`): a value when exact, a bound when `NotAfter` | |
| `Folder<M>` | a folder marker `M` (`Posts`, `Drafts`, `Hidden`, `Obsidian`, `Docs`, `Content`), evidence `Marker { path }` naming the folder | `Mark<C>` on `M` |
| `Pattern<L>` | an `include` or `exclude` pattern | `Listed<L>` |
| `Derived<D>` | the home page's decision of code `D` | `Follows<D>` |
| `Implied<D, W>` | what witness `W` says of code `D` on the same subject | `Follows<D>` |
| `Otherwise<W>` | what `W` says, only when nothing before it said a value | |
| `Fallback<C>` | the default, evidence `Default { reason }` | `Assumed<G>` |

A decision's precedence is a type alias, one per code, in `engine::interpret::decide::precedence`:

```rust
type Title = Ladder<code::Title, (Pin<code::Title>, Front<code::Title>, Heading, Otherwise<FileName<Stem>>)>;
```

Reading the precedence of `page.title` is reading that line, and `ink explain --why x.md
page.title` prints it as it ran. What a code's values mean to a
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

`Journal` is the one sink for decisions (erased as `AnyDecision`, read back typed), diagnostics,
notices and the one trace asked for: nothing is returned on the side, nothing is printed.
Interpret is a composition of stages (`Root`, `Layout`, `Files`, `Read<B, F>`, `Pages`,
`Settings<B>`, `Links`, `Embeds`, `Assets`, `Site`, `Routing`, `Assemble`, `Guard`; the last
refuses, as `page.publish.unstated`, a file a vault or private source would publish without a
stated selection), each generic over what it needs from the host (`B: Blobs`, `P: Pins`,
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
(`fn headers(&self, media: MediaType, cache: Cache) -> Vec<Header>`), so the CSP lives in one
type; quick-xml is seen only by `render::xml`. Before the tuple runs, `Prepare` hands every
raster asset to the host's `Images` at once and gives the site each one's `Picture`, so a theme
knows its size and copies; `Assets` then serves the copies at hashed routes, `Cache::Immutable`
(0040). The engine's render is
`(Pages<T>, NotFound<T>, Assets, Feed<(Atom, Rss)>, Sitemap, Robots, ThemeFiles<T>)` run in
order; adding an output is adding an artifact to that tuple. An artifact that fails is recorded
and the next still runs.

A build keeps a record of itself (`model::record::Build`: its `Inputs`, every decision, every
file's blob, every published file's route), made by `Understood::record` and written by the CLI
beside the manifest in the output folder. `engine::Compared::of(against, before, after)` diffs
two records into `record::Changes`: newly public files, withdrawn and moved routes, and every
decision whose value differs (`AnyDecision::agrees`), each with the first `Cause` that holds:
`Config` when either side's evidence is a pin or pattern, `Plugin` for a format that is another
component, `Ability` for a history-based decision when the source's abilities, or what it lacked of them
(a shallow clone's history), differ,
`Content` when the subject's own file changed, `Config` when `ink.toml` changed, `Content` when
any other file did, else `Engine`, which also gives `engine.decision.unstable`. `Rules` is typed
and given once `site.rules` exists (0042's second step).

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

Link resolution is a `Fold` (unresolved targets become pages), filling embeds is another (each
`Block::Embed` gets the blocks of the page or part it names, a `Carried` fold pointing their
anchors at that page), the summary is a `Visit` that
collects plain text, a theme's HTML is a `Visit` that writes through `unspliced`, outline and
reading time are visits. `ink:theme-quiet`'s `Writer` writes an element with children as a shell
holding `unspliced::HERE`: `Split` writes the shell up to the mark, the walk visits the children,
then the rest is written, so the writer holds `&mut` state (footnote references, the list's
spacing, the record) and never recurses by hand. Footnotes are its `footnote` method, after the
blocks in their own section. A new node kind is added to the model once with its default walk, and
every visitor that does not care keeps working.

## 5. Plugins are traits the engine is generic over

```rust
pub trait Format { fn handles(&self, path: &ContentPath) -> bool; fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read; }
pub trait Formats { fn handles(&self, path: &ContentPath) -> bool; fn read(&self, path: &ContentPath, bytes: &[u8]) -> Option<Read>; }
impl<A: Format, B: Format> Formats for (A, B) { .. }   // tuples up to four: the first that handles it
pub trait Theme { fn name(&self) -> &str; fn assets(&self) -> Vec<File>; fn render<R: Record + ?Sized>(&self, page: &Page, site: &Site, record: &mut R) -> Result<Html, Diagnostic>; }
pub trait Record { fn diagnose(&mut self, diagnostic: Diagnostic); fn notice(&mut self, notice: Notice); }   // model's
pub trait Blobs { fn read(&self, hash: &Hash) -> Result<Cow<'_, [u8]>, BlobMissing>; }
pub trait Pins: Patterns { fn pin<C: Code>(&self, subject: &C::Subject) -> Option<Pin<C::Value>>; }
pub trait Images { type Refusal: Into<Diagnostic>; fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Self::Refusal>>; }
```

The engine is `Engine<B, P, F, T>`, with `interpret` needing the first three and `render` the
blobs and the theme, so a part a call does not use is `()`; `render` also takes the host's
`Images` (`host_image::Pixels` natively, a stand-in in tests); the CLI picks native types, the wasmtime host picks
component adapters implementing the same traits, tests pick in-memory ones. Nothing in the engine
names a concrete plugin. A theme says what it finds while it renders (math that does not parse,
code in a language no grammar knows) into the `Record` it is given, the native stand-in for
`ink:host/diagnostic`: the render's output records it beside everything else, and a test passes
a `model::Said`. `Record`, `Said` and a theme's `File` live in the model, which is
permissive, so the SDK implements `Record` and converts `File` without linking the AGPL engine;
the engine re-exports them.

A dialect of markdown is not a second parser. `format_markdown::dialect::Dialect` says which
syntax comrak parses (`Syntax`: callouts as alerts or folding, and the `Extension`s it turns on:
`^x^` and `~x~`, definition lists, wikilinks, block ids, `%%` comments, `aliases`) and what the dialect's own nodes mean
(`wikilink`, `callout`, `text`, `finish`); `format_markdown::read_as(dialect, path, bytes)` reads
through the one `Builder`, generic over it, so comrak stays in the markdown crate and the raw
HTML allowlist holds for every dialect. `ink:format-obsidian` is the dialect `Obsidian` and
nothing else; the plain format is the dialect `Markdown`. A guest serves a format through
`sdk::Served<F: sdk::Format>`, so the refusals of the boundary are written once.

The component adapters are the same traits over a loaded component: `host_wasmtime::Component<Format>`
implements `Format` and `Component<Theme>` implements `Theme`, so `Engine<B, P, F, T>` runs a
component with no change and `ink build --check` builds a site both ways. A plugin keeps its
native core; its `guest/` crate only converts and calls. The conversions between the model and its
WIT mirror are written once in `wit/mirror/`, as `From` and `TryFrom` impls (a model variant the
WIT lacks is `Unmirrored`, a WIT value the model refuses `Malformed`), and included by both
`host/wasmtime` and `plugin/sdk` over their own generated types: the two generators write the
same shapes and share no type, so inclusion is the one way to write the impls once (decision
0041). A call that fails (a trap, a budget, a refused value) becomes the diagnostic the trait's
signature has room for: a format's read carries it, a theme's render returns it, and what has no
room (a theme's files) is kept aside for the caller to take.

A capability a plugin imports and the engine never calls is a trait in `model`, so the plugin
and the host that implements it meet without going through the engine. Highlighting is the first
(decision 0039):

```rust
pub trait Highlight {
    type Error: std::error::Error + 'static;
    fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Self::Error>>;
}
pub struct Run<'c> { pub capture: Option<Capture>, pub text: &'c str }   // runs cover the code in order
pub enum Unhighlighted<E> { Unknown, Failed(E) }
pub enum Capture { Attribute, Comment, Constant, Function, Keyword, String, Type, .. }   // closed, from tree-sitter's capture names
```

`host_native::Grammars` implements it with arborium; `ink:theme-quiet` is `Quiet<H: Highlight>`
and maps each `Capture` to a class of its code stylesheet in one exhaustive `match`, so a
capture without a class does not compile and no highlighter's markup reaches a page.

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
The rules are `Names`, `Order`, `Licence` (each crate's licence, and no AGPL crate of ours under
a permissive one), `Alias`, `Banned`, `Unsafe` (on tokens, so comments
and literals never count), `Words` and `Free`. `Gradual<R>` wraps a rule so it fails only the
crates in `ENFORCED` and counts the rest in a note; every crate is in it, so a new crate joins
it with its first commit. Lists of generators, steps and rules are tuples, given their list
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
  fixture lists inputs and the expected decision with its evidence, and a trace test lists the
  rungs it expects.
