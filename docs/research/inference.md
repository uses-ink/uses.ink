# How ink decides, and how it accounts for it

Scope: the paradigm behind decisions, evidence, pins and stability across versions of ink. It
asks whether the evidence system earns its place, compares five paradigms, and specifies one to
the level a builder can start from. The decision record it proposes is 0042 (section 8). Sources
are at the end; claims about the code were checked against `main` at `44e19f06` on 2026-11-27.

## 1. Answer first

The purpose of evidence is real; its shape answers the wrong question. Today every value carries
its cause, which answers "why is this value what it is". The jobs that hurt most (something
published by surprise, an upgrade of ink that moves a site, a pull request whose effect nobody
can see, a hosted build nobody watches) all ask a different question: **what changed since the
site I last saw, and why**. A decision record with no memory of the previous build cannot answer
it, however rich each record is.

The proposal, in three parts:

1. **A decision is a value and one piece of evidence.** Rejected alternatives leave the model.
   "Why not my heading?" is answered on demand by a **trace** that replays the ladder, which can
   also say what each rung did *not* find, something today's record cannot say.
2. **Every build is compared with a baseline** (the live deploy hosted, the previous `ink build`
   locally) and reports a **change set** in which each change has a typed **cause**: content,
   `ink.toml`, rules, plugin, source ability, or our bug.
3. **The inference rules are versioned** (`rules = 3`), and the version is itself a decision:
   stated in `ink.toml`, else inherited from the baseline, else the latest. A newer ink never
   changes a live site's decisions on its own; what it would change is shown as **pending**
   until the writer accepts it with `ink update`. This is Rust's editions and Go's GODEBUG
   applied to inference, and it replaces the idea of locking every value.

Pins stay as hand-written facts. `ink explain --pin`, which dumps every inferred value as a pin,
goes: it freezes content into config (section 2.3).

## 2. What is there today

### 2.1 What `ink explain` shows

On `cli/fixture/site/blog` (5 pages, 1 image), `ink explain` prints 69 lines. 27 of them end in
`by default`, and most of those say nothing a writer would ask about (`file.ignore kept`,
`page.tags no tags`, `page.updated never updated`). The lines that matter (`page.title "Later"
from the name of posts/later.md`, the one title that came from a file name) look like every
other line. The "check" flags of `flow.md` section 3 do not exist yet.

### 2.2 What the rejected alternatives do

`Decision<C>` stores `rejected: Vec<Alternative<C::Value>>`, each with a value, its evidence and
a `Rejection` (`Outranked` or `OnlyBound`). Their consumers on `main`:

| Consumer | Uses rejected alternatives |
|---|---|
| `ink explain` (text) | no |
| `ink explain --json` | yes, serialized whole |
| `ink explain --pin` | no |
| engine: the `fact.date.bound` notice (`interpret/page.rs`) | yes, to find the bound |
| dashboard | not built |

So a person never sees them. The one engine use reads a fact (`Known::NotAfter`) back out of
the decision after the ladder put it there. And the JSON carries noise: on a plain folder, every
page's `page.date` and `page.updated` list the checkout's filesystem time as a rejected bound
(`"2026-11-26T23:48:59.874582799Z"`), so `explain --json` changes on every clone and machine.

### 2.3 What `--pin` does to a site

`ink explain --pin` writes every decision as a pin: `title = "About"` for a title that came from
the page's heading, `summary = "..."` for a summary cut from its first paragraph, `ignore =
false` for every file. Pasted into `ink.toml`, it turns content into config: the next edit of
the heading changes nothing, and nothing says so, because a pin outranks the heading by design.
The M1 exit check ("`ink explain --pin` output, put back as `ink.toml`, reproduces the same
deploy") tests exactly this freezing. As a tool for stability it locks the wrong thing: the
outputs of the rules, where an upgrade changes the rules.

### 2.4 Where the lines are

The brief counts about 1,400 of 4,400 engine lines as decision machinery. Measured:

| Part | Lines (code, no tests) | What it is |
|---|---|---|
| `decide/witness/*` | 866 | the inference rules themselves: front matter, headings, names, folders, history, defaults |
| `decide/ladder.rs`, `witness.rs`, `ground.rs`, `precedence.rs` | 416 | precedence as types |
| `decide/explain.rs`, `journal.rs` | 176 | which defaults are announced; the decision sink |
| model `decision*`, `code*` | 808 | typed codes, erased forms, words |

The rules would exist in any design. What only exists for evidence-as-history (`Alternative`,
`Rejection`, the rejection loop, `Standing` as a reason) is about 120 lines. Shrinking a decision
to value plus evidence is worth doing for the model, the WIT boundary, the JSON and the stored
record; it does not shrink the engine by a third, and this document does not claim it will.

### 2.5 What is right and stays

- **Typed codes** (`Decision<C>`, `codes!`): a `page.title` cannot hold a date, and the pin path
  is the code's name (0018). Nothing here changes that.
- **Ladders of witnesses**: precedence is one readable line per code. Keep.
- **`Known<T>`** (0011): Hugo's own docs say a shallow clone makes `.GitInfo` "incorrectly reflect
  the most recent repository commit", and list GitHub Pages, GitLab Pages, Cloudflare, Render and
  Vercel as shallow by default. uses.ink's `NotAfter` turns that silent lie into "undated, and
  said". This is the most useful piece of evidence in the system.
- **Pure interpretation**: it is what makes a trace, a baseline replay and a rules diff cheap.

## 3. The jobs, ranked

Ranked by the cost of failing: what cannot be undone first, then what is frequent.

| # | Job | Who, when | Needs to know | Today |
|---|---|---|---|---|
| 1 | Nothing published by surprise | a vault owner on first publish; anyone after a push or an upgrade | what became public in this deploy, by what rule, and that no rule made it public on its own in a private source | **Partly.** Opt-in for vaults (0017) and the orphan rule (planned) guard the first publish. Nothing shows what *became* public later, and an upgrade that widens a rule would publish silently |
| 2 | An upgrade of ink does not move my site | every hosted site, every time we ship the builder | that live decisions do not change unless I accept; what a newer ink would change | **Not at all.** Hosted sites build with whatever ink the container runs; nothing records which rules built a deploy |
| 3 | A hosted build on a forge, with no local state | a writer who only pushes | after each push: what changed, what is new, what moved, in the commit status | **Badly.** `flow.md` 4.2 sketches "Changed: posts/new.md (added)", a file list; nothing computes changes of decisions or routes |
| 4 | Fix a surprising value | a writer whose title, date or kind looks wrong | where the value came from, the line that changes it, and why a value they expected lost | **Well for the first half.** `ink explain <path>` names the evidence and its span. The "why not" half is never shown, and the default view buries the one doubtful line |
| 5 | Review a pull request of content | a co-author or the writer on a branch | what the change does to the site: pages added, removed, re-titled, URLs moved, newly public | **Not at all** beyond previews |
| 6 | Reproduce a deploy | the owner, an auditor, us debugging | what went in: commit, ink version, rules, plugin digests, config | **Partly.** Deploys are content-addressed (0010) and `BuildEvent::Started` has `inputs`; nothing lets a newer binary reproduce an older deploy's decisions |
| 7 | Write a plugin | a format or theme author | how their testimony ranks, what crosses the boundary, how their version shows up | **Badly.** The code set is closed; a decision on the wire carries rejected alternatives a plugin must fill or ignore |

Jobs 1, 2, 3 and 5 are questions about change between two builds. Only job 4 is a question about
one build, and it is the one today's design was built for.

## 4. Prior art

| System | What it does | Good for our jobs | Bad for our jobs |
|---|---|---|---|
| Cargo.lock | records exact resolved versions; "maintained by Cargo and should not be manually edited"; committed | locks the *inputs* of a build, which change rarely and apart from your code | it locks dependencies. Locking our outputs, which change with every edit of content, is a different thing |
| Nix `flake.lock` | maps each unlocked input to a locked one; `nix flake update` moves it | the same split: intent in `flake.nix`, resolution in the lock | needs a commit to change; our note-takers have none |
| Bazel `MODULE.bazel.lock` | `--lockfile_mode=update` (default), `refresh`, `error`: error "never changes the lockfile" and fails when it is out of date | a CI mode that refuses drift instead of fixing it | three modes for one file is the option creep Prettier warns about |
| Terraform | a dependency lock that "at present tracks only provider dependencies"; `terraform plan` shows the effect before `apply`; state of the last apply is kept remotely | **plan before apply** is our change set; remote state of the last apply is our baseline | `plan` and `apply` are two steps; P4 says publish first |
| Rust editions | incompatible changes go into the next edition, chosen per crate in `Cargo.toml`; `cargo new` writes the newest; `cargo fix --edition` migrates; crates of all editions work together | a version of the *rules*, stated once, opt-in, with a migration tool | an edition every three years batches many changes; ours would be smaller and more frequent |
| Go GODEBUG | a behaviour change gets a named setting; its default comes from the `go` line of `go.mod`; `godebug` lines override one setting; kept at least two years; a counter counts how often the old behaviour was used | **per-change names, defaults from a version, a retention promise**: the closest model to ours | per-setting overrides are a knob per change; we want fewer |
| Prettier | "a few options because of history. But we won't add more"; install with `--save-exact` | few knobs: inference wins by being boring | its stability answer is "pin the binary", which hosted users cannot do, because we run it |
| Black, Ruff | Black: code formatted with it stays unchanged "using any other release in the same calendar year"; Ruff gates new behaviour behind preview mode and changes stable behaviour only in minor versions | a written stability policy with a schedule, and a preview of the next rules | a calendar is coarse; we can name each change instead |
| Elm errors | "Every message has a useful hint"; messages shorten the translation from "the compiler is angry" to "what to do" | explanation is a hint to act on, close to the code | |
| TypeScript `--explainFiles`, `git check-ignore -v` | on request, print why each file is in the build, or which ignore rule matched, with file and line | **why, on demand, for one subject**, including the pattern that matched; nothing is stored | |
| ESLint | a fix is applied by `--fix`; a suggestion is offered and applied only by a person | the line between a change we may make for the writer and one we only propose | |
| Hugo | date sources are a list in config (`date = ["myDate", ":default"]`); `:filename` also sets the slug, but "only when `:filename` is the winning date source" | ladders as data are documented and overridable | coupling between ladders that only the docs explain; shallow clones give wrong `.GitInfo` silently |
| Eleventy | the data cascade merges sources in a documented order | one written order of precedence | the order is prose in the docs; the tool cannot show it for one page |
| Zola, Jekyll | a date in the file name is the page date unless front matter sets one; Jekyll has `_drafts` | the same conventions we infer | no explanation surface at all |
| Astro content collections | a schema (Zod) per collection validates front matter at build | front matter errors are typed and early | the writer writes a schema: config before content |
| Quartz, Obsidian Publish | Quartz's `ExplicitPublish` passes only notes marked `publish`; Obsidian's "Publish changes" dialog lists NEW, CHANGED, UNCHANGED notes; `publish: true` "overrides excluded folders" | **a change list before publishing** is the privacy job done well | the change list lives in the app; a forge push has nothing like it |
| CUE, Dhall | CUE marks a default with `*` inside a disjunction and picks it only when nothing else is given; `dhall freeze` pins an import by the hash of its normal form | defaults as a visible part of the type; integrity checked on the meaning of an import | config languages; our writers write prose |
| SOURCE_DATE_EPOCH, SLSA provenance | reproducible builds take time from the source; SLSA records `buildType`, `externalParameters`, `resolvedDependencies` (such as the commit a URI resolved to) and `builder.id` | a deploy record that says what built it, in fields others know | |

Three lessons carry the design:

1. **Lock inputs, never outputs.** Every lockfile above records what was resolved (versions,
   revisions, hashes), never what the build produced. The thing that moves on an upgrade of ink
   is the rules, so the rules are what gets a version.
2. **Show the effect before it lands, against the last thing that landed.** Terraform's plan
   against remote state and Obsidian Publish's change list are the same idea. Hosted, we already
   have the last thing that landed: the live deploy.
3. **"Why" is a query.** `--explainFiles` and `check-ignore -v` answer it for one subject on
   request, and nobody misses a stored copy.

## 5. Paradigms

Five shapes, each meant to stand on its own.

### A. Evidence-carrying decisions (today)

Every decision stores its value, its evidence and every alternative that lost. `ink explain`
reads the list; pins override.

- **Types**: as built (`Decision<C> { subject, value, evidence, rejected }`).
- **User sees**: `ink explain`, a per-file listing; `--pin`, a whole `ink.toml`.
- **Hosted**: decisions go into the build's event log; the dashboard would read them.
- **Plugins**: a format's decisions cross WIT with rejected lists.
- **Upgrades**: nothing. The advice would be "pin everything", which freezes content (2.3).
- **Privacy**: rules guard the first publish; nothing guards a later one.
- **Cost**: built. Keeping it means a WIT type and a JSON schema with a field nobody reads.
- **Fails when**: ink ships a new ignore rule and every hosted site rebuilds under it with no
  word to anyone.

### B. Lock-and-trace (the coordinator's)

Decision shrinks to value plus one source; alternatives only in a trace. `ink.lock` records every
inferred decision; `ink check` shows what a newer ink would decide differently; `ink update`
accepts.

- **Types**: `Decision<C> { subject, value, evidence }`; `Lock { entries: Vec<(AnyCode, Subject, Value)> }`.
- **User sees**: an `ink.lock` of every title, date, slug and summary; a diff in it on every
  content edit.
- **Hosted**: the lock must be committed to protect anything. The dashboard never writes to the
  source (Q10), and an Obsidian plugin or browser upload has no commit, so the lock is either held
  by us (a second source of truth) or absent, for the audience least able to notice.
- **Upgrades**: covered, if the lock is there.
- **The hole**: a locked value meets edited content. The title is locked as "About" and the
  heading now says "About me". If the lock wins, it is a pin and freezes content. If the content
  wins, the lock locked nothing. Telling the two apart means asking whether the content changed
  or the rules did, which is attribution, which needs no lock at all once a baseline exists.
- **Cost**: small to build, large to keep: every pull request touches `ink.lock`, and merges of
  two content branches conflict in a file nobody wrote.
- **Fails when**: two people edit two posts on two branches; both lock entries change; the merge
  conflicts.

### C. Inference as generated config the writer owns

`ink init` writes everything inferred into `ink.toml` (or a site file), the writer owns it, and
ink infers only for files that are not in it yet. Terraform's `-generate-config-out`, an "eject".

- **Types**: no decisions at build time beyond new files; `Config` is the whole site.
- **User sees**: a long file to keep in step with content: renaming a post means editing two
  places.
- **Hosted**: needs a commit from the first build; the vault path dies (no commit).
- **Upgrades**: stable, because nothing is inferred for known files.
- **Privacy**: good at the start (everything listed), worse later (new files still inferred).
- **Cost**: cheap engine, expensive writer. It breaks the product's promise (zero config) on
  the first run.
- **Fails when**: the writer edits a heading and the title stays, as in 2.3, for every page.

### D. Rules as data the writer can read and override

Ladders become data: `ink rules` prints them, and `ink.toml` can reorder or replace them
(`[rules] page.date = ["front", "first-commit"]`), as Hugo does for dates.

- **Types**: `Ladder` built at run time from `Vec<WitnessId>`; witnesses keyed by name.
- **User sees**: one more place to look; a debate about the right order for each site, which is
  what Prettier refuses to host.
- **Hosted**: fine, it is in the source.
- **Plugins**: could add witnesses by name.
- **Upgrades**: a site that states its ladders is stable for those codes; the rest still moves.
- **Cost**: static ladders become dynamic, typed precedence (`precedence.rs`) is lost, and every
  combination is a test case.
- **Fails when**: a writer copies a ladder from a forum post, and a later default they would have
  wanted never reaches them. The read-only half (`ink rules` printing the ladders) is cheap and
  worth keeping in any paradigm.

### E. Change-attributed builds with versioned rules (proposed)

A decision is a value and its evidence. Every build is diffed against a baseline, and each
change has a typed cause. The rules have a version; a site's version is stated, inherited from
its baseline, or the latest. A newer ink computes what it would change as pending and never
applies it until accepted.

- **Types**: section 7.1.
- **User sees**: a change set in `ink check`, `ink build`, the forge status and the Deploys
  screen; `ink update` to take new rules; `ink explain --why` for one value.
- **Hosted**: the live deploy is the baseline; the control plane already keeps it for rollback.
- **Plugins**: a plugin's digest is an input; a change caused by a new plugin version says so.
- **Upgrades**: held as pending, shown, accepted once per site.
- **Privacy**: "Newly public" heads every change set; in a private source a publish that no
  stated selection made is refused.
- **Cost**: section 7.8; the lasting cost is keeping old rules alive behind named amendments.
- **Fails when**: we change inference behaviour without declaring an amendment. The design makes
  that visible (cause `engine`, our bug) and CI refuses it on the fixture corpus.

### F. Approval-first (Obsidian Publish's model)

Everything inferred is a proposal; nothing is used until the writer approves it once; approvals
are recorded.

- **Good for**: publication in a vault, where it is already the rule (0017, Q16).
- **Bad for**: titles, dates and slugs, where it is a wizard before the first page (against P4).
- **Verdict**: kept where it is (selection of private sources), folded into E as a gate, and
  not generalized.

### Comparison

| | A today | B lock | C generated config | D rules as data | E change-attributed | F approval |
|---|---|---|---|---|---|---|
| Job 1, privacy | first publish only | first publish only | good, then decays | first publish only | **every deploy** | good, slow |
| Job 2, upgrades | no | yes, with a commit | yes | partly | **yes, no commit needed** | yes |
| Job 3, hosted | no | needs a commit | needs a commit | yes | **yes** | needs a UI |
| Job 4, surprising value | yes | yes, with trace | n/a | yes | **yes, with trace** | yes |
| Job 5, PR review | no | a lock diff | a config diff | no | **change set vs live** | no |
| Job 6, reproduce | same binary | lock + binary | config + binary | binary | **record + rules** | binary |
| Zero config kept | yes | yes | **no** | yes | yes | no |
| Churn in the source | none | every edit | every edit | none | **none** | none |
| Build cost | built | small | small | medium | medium | medium |
| Keep cost | low | merges | writer's | test matrix | **amendments** | UI |

## 6. The choice, and the strongest objection against it

E, with A's typed codes and ladders kept, B's trace adopted, D's read-only ladders as a view,
and F kept as the privacy gate for private sources.

Reasons, strongest first:

1. **It serves the jobs that cannot be undone.** A page that went public is crawled and cached;
   "Newly public" at the top of every change set, plus a refusal for unstated publication in
   private sources, is the only design here that guards every deploy after the first as well.
2. **It stabilizes what moves.** Upgrades change rules. A rules version locks the rules and
   lets content flow; a value lock does the reverse and pays for it on every commit.
3. **It needs no state we do not already hold.** Hosted keeps the live deploy for rollback; the
   baseline is that deploy's record. Locally, `ink build` already reads the previous build's
   manifest from the output folder before replacing it. No new file in the writer's source is
   required, so vaults, uploads and forge pushes all get it.
4. **It makes our own mistakes visible.** A decision that changed with the same inputs and the
   same rules is attributed to `engine`, our bug, in the writer's own output. "Nothing is silent"
   applied to us.
5. **It shrinks what crosses boundaries.** A decision on the wire, in WIT, in JSON and in the
   stored record is a value and one evidence; the trace is computed where it is asked for.

**The strongest objection**: keeping old rules alive is a tax that grows with every change, on a
pre-1.0 product that needs to move. Prettier and Black get away with "pin the binary" or "same
calendar year". Why not say "rules change with ink; read the changelog"?

Answer: because hosted users cannot pin the binary. We run it, for every site at once, and we
upgrade it when we choose. "Read the changelog" puts the cost of our upgrade on every writer, who
learns of it from a reader. The tax is bounded three ways. First, an amendment is a named gate in
a witness, read through one method (`case.rules.has::<A>()`), so old behaviour lives as a branch
in one place. Second, amendments are retired on a published schedule (Go keeps a GODEBUG setting
at least two years; we can say the same, or shorter). Third, the policy only gates changes to
decisions: a crash fixed, a new diagnostic or a new format is free. Before launch, `rules = 0`
means "no promise", and the first public release starts at `rules = 1`. The fixture corpus makes
the promise checkable (7.7), so the tax is paid in CI, where it is cheap, instead of in support.

A second objection: an inherited rules version is state on our side, and 0018 says the dashboard
holds no interpretation settings. Answer: the writer never chose it. It is a fact about
a deploy, like which deploy is live, which rollback already keeps on our side. The source can
always state it (`[site] rules = 3`), the stated value wins, and `ink export` writes it. Still,
this bends 0018, so it is an owner question (section 9).

## 7. Specification

### 7.1 Model types

In `model`:

```rust
// decision.rs: rejected alternatives leave the model.
pub struct Decision<C: Code> {
    pub subject: C::Subject,
    pub value: C::Value,
    pub evidence: Evidence,
}

// Evidence keeps its variants. Each has a basis, used by explain, the gates and the change set.
pub enum Basis {
    Stated,     // the writer said it: Config, Frontmatter
    Inferred,   // we read it from something else: Heading, Filename, Body, Marker, History
    Assumed,    // nothing said anything: Default
}
impl Evidence { pub const fn basis(&self) -> Basis; }

// DefaultReason keeps OnlyBound; the bound itself is read from FileFacts, never stored here.
```

The rules version, and the amendments it is made of:

```rust
// rules.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Rules(u16);                      // 0 before launch: no promise
impl Rules {
    pub const LATEST: Self;
    pub fn has<A: Amendment>(self) -> bool { self >= A::SINCE }
}

/// One named change to inference, introduced by a rules version. Declared once by
/// `amendments!`, never by hand; its wording is a catalog entry `rules.amendment.<name>`.
pub trait Amendment: Copy + 'static {
    const NAME: &'static str;               // "templates-anywhere"
    const SINCE: Rules;
    const CODES: &'static [AnyCode];        // the decisions it can change
}
pub enum AnyAmendment { /* one variant per amendment, by amendments! */ }
```

`site.rules` is a decision code like any other, so its precedence, its pin and its explanation
come free:

```rust
codes! { RulesVersion => "site.rules" on Site: Rules; }
// precedence.rs
type RulesVersion = Ladder<code::RulesVersion, (Pin<code::RulesVersion>, Baseline, Fallback<code::RulesVersion>)>;
// Baseline testifies the baseline record's rules, evidence Evidence::Baseline { deploy };
// Fallback testifies Rules::LATEST, evidence Default { reason: NothingMatched }.
```

`Evidence` gains one variant, `Baseline { deploy: DeployId }`, of basis `Inferred`. It is decided
first, before `site.root`, because every other ladder reads it.

What a build records, and what it compares:

```rust
// record.rs
pub struct BuildInputs {                    // the inputs of architecture.md section 15, made concrete
    pub source: Version,                    // commit, upload, rev
    pub snapshot: SnapshotId,
    pub ink: InkVersion,                    // informative; rules decide
    pub rules: Rules,
    pub plugins: Vec<PluginPin>,            // id and digest of every plugin that ran
    pub config: Option<Hash>,               // the ink.toml blob
    pub observed: Observed,
}

pub struct Record {                         // stored beside the manifest, one per deploy
    pub deploy: DeployId,
    pub inputs: BuildInputs,
    pub decisions: Vec<AnyDecision>,
    pub files: Vec<(ContentPath, Hash)>,    // every file of the snapshot and its blob
}

pub enum Against {                          // what a change set compares with
    Deploy { deploy: DeployId },            // hosted live, or fetched by `ink check --against live`
    Folder { manifest: Hash },              // the previous `ink build` in the output folder
    Nothing,                                // first build: every decision is new
}

pub struct ChangeOf<C: Code> {
    pub subject: C::Subject,
    pub before: Option<Decision<C>>,
    pub after: Option<Decision<C>>,
    pub cause: Cause,
}
pub struct Changed;                         // Family: type Of<C> = ChangeOf<C>; erased as AnyOf<Changed>

pub enum Cause {
    Content,                                // the subject's file was added, removed or edited, or the tree moved
    Config,                                 // either side's evidence is Config: a pin or a pattern
    Rules { amendments: Vec<AnyAmendment> },// the rules version differs; the amendments that touch the code
    Plugin { plugin: PluginId },            // the format that read the file has another digest
    Ability { ability: Ability },           // the source gained or lost an ability (history)
    Engine,                                 // same inputs, same rules, different value: our bug
}

pub struct Routes {                         // from the two manifests; cheap, both are hash lists
    pub added: Vec<Route>, pub removed: Vec<Route>, pub changed: Vec<Route>,
    pub moved: Vec<(Route, Route)>,         // the same page under a new route (slug changed)
}

pub struct Changes {
    pub against: Against,
    pub decisions: Vec<AnyOf<Changed>>,
    pub routes: Routes,
    pub pending: Option<Pending>,           // what the latest rules would change, held
}
pub struct Pending { pub rules: Rules, pub decisions: Vec<AnyOf<Changed>> }
```

In `engine`, for explain only:

```rust
pub struct Trace<C: Code> { pub subject: C::Subject, pub rungs: Vec<Rung<C::Value>> }
pub struct Rung<V> { pub witness: WitnessName, pub said: Said<V>, pub chosen: bool }
pub enum Said<V> {
    Value { value: V, evidence: Evidence },
    Bound { value: V, evidence: Evidence },
    Nothing,                                // looked, found nothing: "no `title` key"
}
pub(crate) trait Named { const NAME: WitnessName; }   // every witness: "front matter", "first heading", ...
```

`Testimony` stays as the ladder's working state. It gains an optional trace: the tuple impl
calls `testimony.rung(W::NAME)` before each witness, and a ladder run with tracing on keeps the
rungs. A normal run keeps only the first value. Nothing about `Trace` reaches `model`, WIT or the
record.

### 7.2 Two diagnostics replace what rejected alternatives were for

The one "why not" a writer must see unasked is when two things they *wrote* disagree. The ladder
already hears every witness, so it can say so as it decides:

| Code | Severity | When | Text |
|---|---|---|---|
| `decision.stated.shadowed` | warning | a `Stated` testimony loses to another `Stated` one with a different value | `posts/x.md: title "Shelves" in front matter is overridden by ink.toml line 12 ("On shelves").` |
| `config.pin.redundant` | notice | a pin's value equals what the next testimony says | `ink.toml line 12 pins page.title of posts/x.md to what its heading already says; the line can go.` |

The `fact.date.bound` notice reads the bound from `FileFacts` through the decision's subject.

### 7.3 Data flow

```
                      baseline record (live deploy, or the out folder's manifest), optional
                                   |
snapshot + ink.toml + plugins -----+----> interpret(rules R) ---> decisions D ---> render ---> manifest M
                                   |            ^                                             |
                                   |      site.rules decided first:                          |
                                   |      pin, else baseline, else latest                    |
                                   |                                                          v
                                   +----> interpret(Rules::LATEST), only when LATEST > R ---> pending
                                   |
                                   +----> diff(baseline, D, M) with causes ---> Changes ---> gates ---> publish or hold
```

Interpreting twice costs the interpret stage only (no render); on the fixtures it is
milliseconds, and on a 40,000-file tree it is to be measured (section 10, experiment 2).

Cause, per changed decision, first match wins:

1. The rules differ (`R != baseline.rules`, after an accepted update): replay interpret under
   the baseline's rules on the current snapshot; a decision that matches the baseline there and
   differs here is `Rules`, with the amendments whose `CODES` hold its code.
2. Either side's evidence is `Config`: `Config`.
3. The plugin that read the subject's file has another digest: `Plugin`.
4. The source's observed abilities differ and either side's evidence is `History` or
   `Default { OnlyBound }`: `Ability`.
5. The subject's file is new, gone or has another blob, or (for site decisions) the tree
   differs: `Content`.
6. Otherwise: `Engine`, with the diagnostic `engine.decision.unstable` (fault `Ours`).

Route changes take their cause from the decisions behind them: a moved route follows its
`page.slug` change; a changed route with no changed decision and no changed blob is the theme's
(another digest) or ours (`Engine`).

### 7.4 Gates

Which changes stop a deploy from going live on its own. Everything else publishes, as P4 says.

| Gate | Condition | Effect |
|---|---|---|
| Unstated publication | the source is private or a vault, and a page or file is published by a decision whose basis is not `Stated` (directly, or through the page that uses it) | error `publish.unstated`; the deploy is refused. By the ladders this cannot happen; the gate is the invariant written down, so a bug cannot leak |
| Rules | `LATEST > R` | never applied; shown as pending in every surface until `ink update` |
| Engine | any change with cause `Engine` | a warning; holding the deploy would punish the writer for our bug; the warning links the report page |

`Newly public` (routes added whose page was not public before) heads every change set for every
source, public or private, so job 1 is visible even where it does not block.

### 7.5 Files

| File | Who writes it | What changes |
|---|---|---|
| `ink.toml` | the writer; `ink pin`, `ink update` and `ink select` with `toml_edit` | gains `[site] rules = 3`, written by `ink update` and `ink init` (as `cargo new` writes `edition`); never required |
| `ink.lock` | the CLI | as planned (architecture.md section 6): third-party plugin digests. No decisions, no rules version |
| output folder manifest (`host_native::Folder::MANIFEST`) | `ink build` | gains the `Record` (inputs and decisions), so the next `ink build` and `ink check --against out` have a baseline; it is already a dotted file that is never a route |
| deploy record blob | the builder | one per deploy, beside the manifest blob, retained with it (the last 20 and the live one); the live deploy's snapshot tree is retained with it so `Content` can be told apart |

### 7.6 CLI and surfaces

`ink explain` shows what is worth reading by default, and all of it with `--all`:

```
$ ink explain
the site        a blog (posts/ is there)           rules 1 (latest; nothing to inherit)
                home index.md   title "Ink and paper" (front matter)   language en (default)

posts/later.md  not published (front matter publish: false)
                title "Later"   from the file name   check: no heading and no title key
posts/second.md title "Paper weight" (heading)   2024-04-02 (front matter)
...
28 more decisions are defaults with nothing to say: ink explain --all
```

`--why` replays one decision's ladder, rung by rung:

```
$ ink explain --why posts/later.md page.title
posts/later.md  page.title = "Later"
  1 ink.toml          nothing     no [page."posts/later.md"] title
  2 front matter      nothing     no title key (lines 1-3)
  3 first heading     nothing     the file has no level-1 heading
  4 file name         "Later"     chosen
To set it: add `title: ...` to the front matter, or a `# Heading` as the first line.
```

`ink check` and `ink build` end with the change set when there is a baseline:

```
$ ink check
against   the previous build in _site (d_8a1m, rules 1)
newly public
  posts/new.md          /posts/new/        new file
changed
  posts/second.md       title "Paper weight" -> "On paper weight"     content (heading edited)
  posts/old.md          route /posts/old/ -> /posts/older/            content (slug in front matter)
                        the old address will not be served
  about.md              date undated -> 2023-02-10                   source: history is now available
pending (rules 2, held until `ink update`)
  templates/post.md     ignored                                      amendment templates-anywhere
1 newly public, 3 changed, 1 pending. 0 errors, 1 warning.
```

`ink update` shows the pending changes, writes `rules`, and updates `ink.lock`'s plugin
digests, each with what it would change:

```
$ ink update
rules 1 -> 2
  templates-anywhere   a folder named templates is ignored at any depth (today: only at the top)
    templates/post.md       kept -> ignored
plugins: none changed
Write rules = 2 to ink.toml? [y/N] y
Wrote [site] rules = 2 to ink.toml.
```

`ink update --dry-run` prints the same and writes nothing; `--yes` skips the question.

`ink pin <path> <code>` and `ink pin --site <code>` write one pin of the current value with
`toml_edit`, and print the line. `ink explain --pin` and `--minimal` are removed.

`ink rules` prints the ladders (precedence.rs, worded) and the amendments of each version:
paradigm D's useful half, as a view.

JSON: `uses.explain/v2` (decisions without `rejected`), `uses.trace/v1`. The change set has no
schema of its own: it is a field of `uses.build/v2`.

Hosted surfaces carry the same change set:

```
uses.ink / publish   success: deploy d_9b2n, 24 pages
Newly public: posts/new.md -> https://cstef.uses.ink/posts/new/
Changed: posts/second.md title (content), posts/old.md route /posts/old/ -> /posts/older/ (content)
Pending: rules 2 would ignore templates/post.md. Run `ink update` or accept on the Site screen.
```

On a pull request branch, the preview's check compares with the live deploy, so a reviewer
sees what merging does, whatever earlier previews of the branch showed. The Deploys screen shows each deploy's
change set against the one before it, so "when did this file become public, and why" has an
answer for every file in the retained history.

Accepting pending rules hosted, without a commit: a Site-screen action "Publish with rules 2"
builds the same commit under the newer rules. That deploy's record says `rules 2`, and later
builds inherit it. The screen says that writing `[site] rules = 2` in `ink.toml` makes the choice
part of the source, the same wording as the held selection of 2.6 Path C.

### 7.7 Policy for amendments

- A change to the engine or a bundled format that alters any decision of any fixture under an
  existing rules version is an amendment, or it is a bug fix listed as one in the changelog.
  Allowed bug fixes: a crash, an error where a value was owed, a decision that contradicted its
  own documented ladder.
- `cargo xtask check` keeps a decision golden per fixture per rules version
  (`cli/fixture/site/<name>.rules-<n>.snap`). A change to an old version's golden fails unless
  the commit names a bug-fix entry.
- Amendments are retired, with their old behaviour, after a period the owner sets (section 9);
  a site still on a retired version is built with the oldest kept version, and its change set
  says so once.
- Bundled themes and render changes are not amendments: they change bytes and leave decisions alone, and
  show up as changed routes with their cause.

### 7.8 Migration from today's code, in order

Each step leaves `cargo xtask check` green.

1. **Trim the decision.** Delete `Alternative`, `Rejection`, `Decision::rejected` and
   `Decision::rejecting` from `model/src/decision.rs`; the rejection loop in `Ladder::decide`
   picks the first `Standing::Value` and drops the rest. Rewrite `Undated::announce`
   (`interpret/page.rs`) to read the bound from `FileFacts`. Add `Basis`. Bump `uses.explain`
   to v2. Snapshots change. About 120 lines go.
2. **Add the two diagnostics** of 7.2 in `Ladder::decide`, with catalog entries.
3. **Trace.** `Named` on every witness, the optional trace in `Testimony`, `Trace<C>`, and
   `ink explain --why`. The default view of `ink explain` hides defaults with nothing to say
   (the engine's `Explain` already knows which defaults are announced) and gains `--all`.
4. **Replace `--pin` with `ink pin`.** `config::pins` stays as the writer of one or more pins;
   `cli/src/pins.rs` is reworked around a single decision. Replace the M1 exit check "explain
   --pin reproduces the deploy" with "a second build against the first as baseline reports no
   change, and the deploy id is equal".
5. **Record.** `BuildInputs` and `Record` in `model`; `ink build` writes the record into the
   output folder's manifest file; `Against::Folder` read back from it.
6. **Changes.** The diff and the cause order of 7.3 for content, config, plugin, ability and
   engine; `Routes` from two manifests; the change set in `ink check` and `ink build`.
7. **Rules.** `Rules`, `Amendment`, `amendments!`, the `site.rules` code and its ladder,
   `Evidence::Baseline`, `rules` on `Case`; `Rules::LATEST = Rules(0)` until launch. The pending
   run, `ink update`, `ink rules`. Per-version goldens in xtask.
8. **Hosted** (with M3 and M4): the record blob per deploy, retention of the live snapshot tree,
   the baseline passed in the build request, the change set in forge statuses and the Deploys
   screen, "Publish with rules n" on the Site screen, the `publish.unstated` gate.

Kept as is: `Code`, `codes!`, `AnyDecision`, `AnyOf`, `Family`, `Each`, `Typed`, `Ladder`,
`Witness`, every witness, `precedence.rs`, `Known<T>`, `Provenance`, `Observed`, `Lack`,
`Journal`, `Explain`, `Word`, pins and patterns in `config`, the deploy id as the manifest hash.

## 8. Decision record (proposed)

> ## 0042 Decisions carry one evidence; changes carry a cause; inference rules are versioned
>
> Accepted as 0042 (see decisions.md for the owner's answers). A decision is its value and the evidence that chose it; rejected alternatives leave
> the model, and `ink explain --why` replays a ladder on request, rung by rung. Every build is
> compared with a baseline (the live deploy hosted, the previous `ink build` locally) and reports
> a change set in which each change has a typed cause (content, config, rules, plugin, ability,
> engine). Inference rules have a version, decided as `site.rules`: a pin, else the baseline's,
> else the latest. A newer ink never changes a site's decisions on its own: what its rules would
> change is shown as pending until `ink update` or a dashboard action accepts it. A private
> source refuses any publication that no stated selection made. `ink explain --pin` is removed;
> `ink pin` writes one pin. **Why:** the jobs that cannot be undone (a page made public, a site
> moved by our upgrade) are questions about change between builds, which per-value evidence
> cannot answer; locking rules instead of values keeps content free and needs no commit, so
> vaults and uploads are covered; the baseline is state we already keep for rollback.
> **Reversal:** amendments turn out to be needed more than about once a quarter, or replaying
> interpret under two rules costs more than 10% of build time on the largest fixture: then
> drop pending replays and fall back to a published rules changelog with the change set alone.
> Refines 0011 (bounds stay facts), 0018 (the rules version is the one value a deploy may hold when the
> source states none) and the M1 exit criteria.

## 9. Questions for the owner

Answered in 0042; the defaults below were the proposal's.

| # | Question | Default |
|---|---|---|
| E1 | May a deploy hold the rules version when `ink.toml` states none (bends 0018)? | yes, labelled on the Site screen as held outside the source |
| E2 | How long are old rules kept? | two years after the version that replaced them |
| E3 | Do bundled theme updates go live without acceptance? | yes: they change looks, never what is public or where |
| E4 | Does the first hosted deploy suggest committing `[site] rules = n`? | it says so once on the Site screen; nothing is written (Q10) |
| E5 | Are `Engine` changes a warning or a hold? | a warning: the writer did nothing wrong |

## 10. What I am unsure of, and the experiment that settles it

1. **How often rules change.** If amendments are rare, the tax is small; if every week, rules
   versions become noise. *Experiment*: replay the engine's history since the code rework over
   the six site fixtures plus one real vault and one real notes repo, and count commits that
   change a decision. Under one per month: as specified. Above: batch amendments per release.
2. **The cost of interpreting twice.** *Experiment*: time interpret alone on tldr (40,944
   files, already used in 0036) under one rules version, then two. Over 10% of the build:
   compute pending only in `ink check`, `ink update` and a daily hosted job, never per push.
3. **Whether the hosted dashboard needs stored traces.** `--why` replays from the snapshot,
   which the CLI has and the dashboard does not. *Experiment*: measure the record size with full
   traces on the real vault. Under about 5% of the deploy's bytes: store traces in the record.
   Above: the dashboard asks the builder to replay, which needs the retained snapshot.
4. **Whether `Content` versus `Config` attribution holds for site-level codes** (`site.nav`,
   `site.layout`) whose input is the tree. *Experiment*: a fixture suite of paired edits (move a
   file into `posts/`, add `[site] nav`, both in one commit) with expected causes as goldens.
5. **Whether writers read the change set.** Obsidian Publish's dialog says people use a change
   list when it sits in the publishing path. *Experiment*: in the M3 dogfood, put "Newly public"
   in the forge status summary and ask three writers after a week what they last saw there.
6. **Local and hosted parity on dates.** A git checkout built locally reads mtimes (bounds);
   hosted reads history (exact). The change set reports the difference as `Ability`, which is
   honest and still surprising. *Experiment*: let `host/native` read history when the folder is
   a git work tree, and see whether any fixture's dates then differ between the two.

## Sources

- Cargo, "Cargo.toml vs Cargo.lock": https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html
- Nix, `nix flake` (lock files): https://nix.dev/manual/nix/2.24/command-ref/new-cli/nix3-flake.html
- Bazel, "Bazel Lockfile" (`--lockfile_mode`): https://bazel.build/external/lockfile
- Terraform, "Dependency Lock File": https://developer.hashicorp.com/terraform/language/files/dependency-lock
- Terraform, `terraform plan`: https://developer.hashicorp.com/terraform/cli/commands/plan
- Rust, "What are Editions?": https://doc.rust-lang.org/edition-guide/editions/index.html
- Go, "Go, Backwards Compatibility, and GODEBUG": https://go.dev/doc/godebug
- Prettier, "Option Philosophy": https://prettier.io/docs/option-philosophy and "Install" (`--save-exact`): https://prettier.io/docs/install
- Black, "Stability Policy": https://black.readthedocs.io/en/stable/the_black_code_style/index.html
- Ruff, "Versioning" and "Preview": https://docs.astral.sh/ruff/versioning/ and https://docs.astral.sh/ruff/preview/
- Elm, "Compiler Errors for Humans": https://elm-lang.org/news/compiler-errors-for-humans
- TypeScript, `explainFiles`: https://www.typescriptlang.org/tsconfig/#explainFiles
- git, `git check-ignore`: https://git-scm.com/docs/git-check-ignore
- ESLint, "Custom Rules" (fixes and suggestions): https://eslint.org/docs/latest/extend/custom-rules
- Hugo, "Configure front matter" (dates, `:filename`): https://gohugo.io/configuration/front-matter/
- Hugo, `GitInfo` (shallow clones by host): https://gohugo.io/methods/page/gitinfo/
- Eleventy, "Data Cascade": https://www.11ty.dev/docs/data-cascade/
- Zola, "Page" (dates from file names): https://www.getzola.org/documentation/content/page/
- Jekyll, "Posts" (`_drafts`): https://jekyllrb.com/docs/posts/
- Astro, "Content collections": https://docs.astro.build/en/guides/content-collections/
- Quartz, `ExplicitPublish`: https://quartz.jzhao.xyz/plugins/ExplicitPublish
- Obsidian, "Publish your content": https://github.com/obsidianmd/obsidian-help/blob/master/en/Obsidian%20Publish/Publish%20your%20content.md
- CUE, "Default Values": https://cuelang.org/docs/tour/types/defaults/
- Dhall, "Safety guarantees" (`dhall freeze`): https://docs.dhall-lang.org/discussions/Safety-guarantees.html
- Reproducible Builds, `SOURCE_DATE_EPOCH`: https://reproducible-builds.org/docs/source-date-epoch/
- SLSA, "Provenance" v1.0: https://slsa.dev/spec/v1.0/provenance
