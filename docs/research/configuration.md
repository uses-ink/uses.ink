# Configuration inventory

Every fixed value in the workspace that a reader could reasonably ask to change, sorted into the
split decision 0044 records. Written 2026-11-27, after the owner's remark that magic values and
limits were hardcoded. `spikes/` is left out.

## How it was found

Every `const` and `static` in every crate's `src/` was read, then every numeric literal in
non-test code under `engine`, `cli`, `config`, `model`, `plugin`, `host`, `diag`, `fixture` and
`token` (including `Duration`, shifts, `take`, `clamp` and `available_parallelism`), then the
header strings of `render/policy.rs`. Inline values that are not consts and were found that way:
the table limit `1 << 16` (wasmtime), the decoder allocation cap `pixels * 16`, the AVIF job
weight `20`, the animation probe `take(2)`, the encoder clamps, the entity lookahead `12`, and
the `explain` column widths. Test files, benches and generated files were skipped.

## The buckets

| Bucket | Who sets it | May `ink.toml` change it | Rows |
|---|---|---|---|
| 1 Site owner | the site's author, in `ink.toml` | yes, it is the point | 6 |
| 2 Operator or host limit | whoever runs the build: CLI flag, host config | only lower it, never raise it | 27 |
| 3 Never configurable | nobody: identity, protocol, security policy | no | 53 |
| 4 Inference rule | 0042's versioned rules (M4) | no, it is not a setting | 7 |
| 5 Tooling | the repository | not applicable | 4 |

The rule behind bucket 2: a source is untrusted, so a source must not be able to raise what the
builder spends on it. A pixel budget, a fuel budget or a cache size in `ink.toml` could only be
lowered by the site; the operator's value is the ceiling.

A bucket 3 value is fixed because changing it changes an identity (a URL, a hash, a wire
format), weakens a security policy, or is a limit of a format or of an OS. Where the reason is a
hash input, the value is fixed for every site at once, and a change is a new version of the
thing it names, never a setting.

Bucket 1 has six rows because the owner's list is short: three image numbers, the picture `sizes`
hint, the theme's style tokens, and description lengths. The description lengths are an owner
call, noted in their row.

## Inventory

Bucket numbers are the ones above. "Built" means a typed struct with a `Default` equal to the
old value now exists and is passed through; no CLI flag or `ink.toml` key exists yet.

### host/image

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| widths 480, 960, 1440 | `settings.rs` `Settings::default` | 1 | `ink.toml` `[image] widths`. Built as `Settings.widths`; in `Key`. |
| AVIF quality 70 | `settings.rs` `Settings::default` | 1 | `[image] avif-quality`. Built as `Settings.avif_quality`; in `Key`. |
| JPEG quality 80 | `settings.rs` `Settings::default` | 1 | `[image] jpeg-quality`. Built as `Settings.jpeg_quality`; in `Key`. |
| AVIF speed 9 | `settings.rs` `Settings::default` | 2 | Operator: it trades builder CPU for bytes (0040 reversal). In `Settings` and `Key`. A site may only choose a cheaper (higher) speed. |
| pixel budget 50,000,000 | `settings.rs` `Limits::default` (was `Settings.pixels`) | 2 | Built as `Limits.pixels`; in `Key` so a lowered budget refuses a cached image. Error `Reason::Budget` carries width, height and limit. |
| workers 8 (`WORKERS`) | `settings.rs` `Limits::default` (was `Pixels::WORKERS`) | 2 | Built as `Limits.workers`. Changes no byte; not in `Key`. |
| images opened at once 2 (`OPENING`) | `settings.rs` `Limits::default` (was `Run::OPENING`) | 2 | Built as `Limits.opening`. Changes no byte; not in `Key`. |
| decoder allocation cap, pixels times 16 | `decode.rs` `max_alloc` | 2 | Derived from the pixel budget (16-bit RGBA and the decoder's own copy); follows `Limits.pixels`, no knob of its own. |
| job weight: an AVIF costs 20 times a JPEG | `work.rs` `Job::weight` | 3 | Scheduling order only; changes no byte. A tuning number, not a policy. |
| ladder rule: a step is kept when it is under 90% of the width | `settings.rs` `Settings::ladder` | 3 | Part of how `widths` is read; changing it changes which copies exist for every site. Bump `Key::VERSION` if it ever changes. |
| `Key::VERSION` string and crate versions | `settings.rs` | 3 | Names the work; bumped by hand when an encoder or the work changes. Identity of cached bytes. |
| record `START` `uses.ink/image/record/v1` | `record.rs` | 3 | On-disk cache record format. |
| thumbhash `SIDE` 100 and `DRAWN` 32 | `thumbhash.rs` | 3 | Changes the placeholder bytes. Covered by `Key::VERSION` only, so making it a setting means adding it to `Key` first. Not proposed. |
| animation probe reads 2 frames | `decode.rs` | 3 | Decides kept-as-is versus re-encoded; fixed by 0040. |
| encoder clamps: speed 1 to 10, JPEG quality 1 to 100 | `encode.rs` | 3 | The encoders' valid ranges. Surprise: they clamp silently. Keys arriving from `ink.toml` must be refused out of range with a typed error before they reach here (see 0044). |
| Catmull-Rom filter, PNG `Best` and `Adaptive`, JPEG progressive, rav1e on 1 thread | `encode.rs` | 3 | Determinism and bytes (0040). Not settings; `Key::VERSION` covers them. |
| JPEG side at most 65,535 | `encode.rs` | 3 | A limit of the JPEG format; `Reason::Large` carries the size. |

### host/wasmtime

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| fuel 10,000,000,000 | `budget.rs` `Budget::default` | 2 | Built as `Budget.fuel`. `Failure::Fuel` carries the limit. |
| wall time 2 s | `budget.rs` `Budget::default` | 2 | Built as `Budget.time`. `Failure::Time` carries the limit. |
| memory 64 MiB an instance | `budget.rs` `Budget::default` | 2 | Built as `Budget.memory`. `Failure::Memory` now carries the limit and the size asked for. |
| table 65,536 elements (inline `1 << 16`) | `capability.rs` `table_growing` (was a literal) | 2 | Built as `Budget.table`. New `Failure::Table` carries the limit and the size asked for; reported as `plugin.call.trapped`, as before. |
| epoch `TICK` 5 ms | `budget.rs` (was `Budget::TICK`) | 2 | Built as `Budget.tick`. A zero tick is `Error::Tick`. |
| fuel and epoch interruption switched on | `runtime.rs` | 3 | The mechanism that makes budgets work; not optional. |
| `.cwasm` file: 32-byte BLAKE3 prefix, name `<hash>-<compat>.cwasm` | `cache.rs` | 3 | On-disk format and the one `unsafe` of 0035. |
| `.cwasm` folder (`--components` cache) | `cache.rs`, `cli` | 2 | Already a path chosen by the caller; not a limit. |

### host/native

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| files 20,000 | `budget.rs` `Budget::default` | 2 | Built as `Budget.files` (read-side struct, existing). Typed refusal with count and limit. |
| one file 25 MiB | `budget.rs` | 2 | `Budget.file`, existing. |
| all files 1 GiB | `budget.rs` | 2 | `Budget.total`, existing. |
| folders 20,000 | `budget.rs` | 2 | `Budget.folders`, existing. |
| cache `BOUND` 2 GiB | `cache.rs` `Disk::BOUND` | 2 | Built as the existing public `Disk.bound`, now also `Disk::with_bound`. Sweeping is the limit; `cache.entry.evicted` carries it. |
| git diff threads, up to 4 | `git.rs` `Limits::default` (was `Repository::THREADS`) | 2 | Built as `GitLimits.threads` in `Repository.limits`. The dates are the same on any number (tested). |
| git delta-base cache 8 MiB a thread (`DELTAS`) | `git.rs` `Limits::default` (was `Repository::DELTAS`) | 2 | Built as `GitLimits.deltas`, non-zero (gix panics on zero). |
| link hops 40 (`HOPS`) | `git.rs` `Limits::default` (was `Links::HOPS`) | 2 | Built as `GitLimits.hops`. Decides whether a chain resolves, so it is output-affecting. New `Refused::Looped` carries path, target and limit; same diagnostic `source.link.broken`. |
| diff block 256 commits (`BLOCK`) | `git.rs` `Limits::default` (was `history::BLOCK`) | 2 | Built as `GitLimits.block`, non-zero. Changes no byte. |
| LFS pointer at most 1024 bytes (`MOST`) | `read.rs` | 3 | The git-lfs spec's own bound on a pointer file. |
| LFS pointer `START` | `read.rs` | 3 | Protocol. |
| skipped folders `.git`, `.jj` | `folder.rs` `VERSION_CONTROL` | 3 | What a version-control folder is. Site-level skips are `ink.toml` `exclude`. |
| folder snapshot `PREFIX` `uses.ink/folder/v1` | `folder.rs` | 3 | Hash input; changes every snapshot id. |
| `MANIFEST` `.ink-manifest.json` and its `MARK` | `folder.rs` | 3 | How a build's own output is recognised and skipped. |
| cache `LABEL`, `MARK`, `ENTRY` | `cache.rs` | 3 | On-disk format and how a cache is recognised and skipped. |
| cache layout `value/` and `key/<kind>/<digest>` | `cache.rs` | 3 | On-disk format. |
| `NONBLOCK` flag value per target | `handle.rs` | 3 | An OS constant. |
| gix opened isolated, object cache 0 | `git.rs` | 3 | A user's or system's git config must not change a snapshot. |

### cli

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| output folder `.ink/out` | `site.rs` `DEFAULT_OUT` | 2 | A default for an existing flag (`--out`). |
| cache folder `.ink/cache` | `cache.rs` `Caching::DEFAULT` | 2 | A default for the existing `--cache` flag. |
| preview port 4000 | `args.rs`, `site.rs` `DEFAULT` | 2 | A default for the existing `--port` flag. |
| preview bound to 127.0.0.1 | `preview.rs` | 3 | Preview serves unreviewed output; loopback only is a security choice. |
| `ink.toml` file name | `site.rs` `CONFIG`, `engine` `name.rs` `CONFIG` | 3 | Identity of the config file. Two copies: a one-source-of-truth item for another change. |
| schemas `uses.error/v1`, `uses.manifest/v3`, `uses.event/v1` | `output.rs`, `out.rs`, `report.rs` | 3 | Wire formats. |
| `explain` column widths 18 and 24, hash cut at 12 | `explain.rs` | 3 | Presentation of one report. |
| 404 file name `404.html` | `out.rs` | 3 | Output layout. |

### engine

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| `Cache-Control: public, max-age=31536000, immutable` | `render/policy.rs` `IMMUTABLE` | 3 | Routes hold the hash of their bytes, so the bytes never change; any other value is wrong or wasteful. |
| page and file header sets (CSP, CORP, COOP, permissions, referrer, nosniff, frame) | `render/policy.rs` `PAGE`, `FILE` | 3 | Security policy (architecture section 13). A weaker header would be a hole a source could ask for. |
| route hash `DIGITS` 16 | `render/picture.rs` | 3 | Part of every copy's URL and so of the deploy hash. |
| description cut near 160, sentence end from 80 up to 240 | `interpret/decide/witness/body.rs` `NEAR`, `SHORTEST`, `LONGEST` | 1 | Owner's call: a site-owner setting (`[site] summary-length`) or a versioned rule of 0042 (M4), not both. It changes a decision, so a change is cause `config` or `rules`. |
| `WRITTEN = "en"` | `interpret/decide/witness/fallback.rs` | 4 | Inference rule, 0042's versioned rules. |
| content folders `content`, `docs`, `notes`, `src/content` | `interpret/tree.rs` | 4 | Inference rule. |
| home stems `index`, `README`, `readme`, `Home`, `Index` | `interpret/site.rs` | 4 | Inference rule. |
| changelog stems, licence stems | `interpret/decide/witness/name.rs` | 4 | Inference rule. |
| post folders `posts`, `blog`, `writing` | `witness/folder.rs` | 4 | Inference rule. |
| vendored `node_modules`, template folder `templates`, `.obsidian` | `witness/folder.rs` | 4 | Inference rules. |
| file index names `index`, `readme` | `interpret/file/name.rs` | 4 | Inference rule. |
| dated file name prefix length 10 (`YYYY-MM-DD`) | `interpret/file/name.rs` | 3 | The shape of an ISO date. |
| raster types the host can prepare | `image.rs` `RASTER` | 3 | What `host/image` decodes; the two change together. |
| XML namespaces Atom and sitemap | `render/feed.rs`, `render/sitemap.rs` | 3 | Protocol. |
| generated routes `/404`, feeds, sitemap, robots | `render.rs` | 3 | Output identity. |

### model

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| `Hash::DIGITS` 64, hash 32 bytes | `hash.rs` | 3 | Identity of every blob and deploy. |
| `SLUG_DIGITS` 12 | `route.rs` | 3 | Changes URLs. |
| manifest `PREFIX` `uses.ink/manifest/v2` | `manifest.rs` | 3 | Hash input of every deploy. |
| `UNITS` and 1024 in `Bytes` display | `size.rs` | 3 | Display of sizes; the unit names are the product's words. |
| git object id 40 or 64 hex digits; upload id 1 to 64 characters | `snapshot/version.rs` | 3 | Protocol of a snapshot's version. |
| status codes 200 and 404 | `manifest.rs` | 3 | HTTP. |
| `none`, `undated`, `never updated`, `no tags`, `no one`, `a generated listing`, `empty` | `code/word.rs` | 3 | Wording of decision values in reports. |

### config

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| `Shape` consts, `LAYOUTS`, `KINDS` names | `value.rs` | 3 | The schema, generated from the decision codes (architecture section 10). New keys join here, not elsewhere. |
| TOML year range 0 to 9999 | `write/written.rs` | 3 | A limit of TOML. |

### plugin/format/markdown

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| `DEPTH` 64 | `error.rs` | 2 | List only (not this change): nesting after which a block or inline keeps only its text. A plugin limit; the host could pass it later. |
| tag `NAME` 64 | `html.rs` | 2 | List only: longest tag name kept. |
| entity lookahead 12 characters (inline literal) | `html/text.rs` | 2 | List only: how far a `&` looks for its `;`. Not a const; found by reading. |
| `OPAQUE` `script`, `style` | `html.rs` | 3 | Security policy: content of these is code, never prose. |
| HTML allowlist, `details`, `summary` | `body/element.rs`, `body/html.rs` | 3 | Security policy (0027). |
| footnote name `DIGITS` 8, heading fallback `section` | `body/name.rs` | 3 | Changes anchors in the rendered HTML and so the deploy hash. |
| front matter key names (`draft`, `lang`, `name`, `alias`, ...) | `front/field.rs`, `front.rs` | 3 | The format's contract with authors. |
| byte order mark `U+FEFF` | `file.rs` | 3 | Encoding. |
| plugin ids `ink:format-markdown`, `ink:theme-quiet` | guests | 3 | Identity. |

### plugin/theme/quiet

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| picture `sizes` hint `(min-width: 42rem) 40rem, calc(100vw - 2rem)` | `document/picture.rs` `SIZES` | 1 | A theme token, as the owner asked: the theme declares it, `ink.toml` `[theme]` sets it. Must match the theme's own measure. |
| colours, fonts, measure and spacing in the style sheet | `style.rs` | 1 | Theme-owned; becomes site-settable only as tokens the theme declares (0041 tokens). |
| MathML allowlist: `ELEMENTS`, `UNWRAPPED`, `ATTRIBUTES`, `DROPPED` | `document/math/allow.rs` | 3 | Security policy: what converted MathML may contain. |
| asset routes `/style.css`, `/code.css`, `/feed.xml`, `/rss.xml` | `page.rs` | 3 | The theme's output names. |

### freehand

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| `MOST` 100,000 points a line | `centreline.rs` | 2 | Library safety cap (a longer line draws nothing). List only; a host surface would need a parameter on the pen's API. |
| `STEP` 5.0 | `centreline.rs` | 3 | Changes every drawn line; goldens. |
| outline constants (caps, pressure, streamline, noise, radius) | `tool/outline.rs` | 3 | A port of perfect-freehand; every value is the original's. |
| `LARGEST` 1e12 | `path/tenths.rs` | 3 | Keeps sums of tenths exact. |
| FNV `BASIS` for seeds | `chance.rs` | 3 | Seeds are hashes of text; changing it redraws every mark. |

### tooling and tests (out of scope)

| Value | Where | Bucket | Home, or why it stays fixed |
|---|---|---|---|
| nextest budgets 100, 250 and 500 ms | `.config/nextest.toml` | 5 | Test policy, not product. |
| fixture corpus words, sizes and seeds | `fixture/src/corpus.rs` | 5 | Test data. |
| xtask lists: `ENFORCED`, `Unsafe::ALLOWED`, guests, directories, skips | `xtask/` | 5 | Repo tooling. |
| generated tables | `token/`, `model/src/generated.rs`, `diag/` | 5 | Generated from `token.toml` and `diag/catalog.toml`. |
## Findings worth a second look

- **The image `Key` already held every output-affecting setting** (widths, AVIF quality, AVIF
  speed, JPEG quality, and the pixel budget). It hashed `pixels` from `Settings`; that field is
  now `Limits.pixels`, hashed identically, so no cache key on disk changed. A test destructures
  `Settings` whole, so a new field breaks the build until it is in `Key` and in the test.
- **The pixel budget is in the key on purpose.** Without it, a lowered budget would not refuse an
  image an earlier build kept, since the cache is read before the decoder sees the header. Tested.
- **Silent clamps in `encode.rs`** (speed, JPEG quality) hide an out-of-range setting. Today
  nothing can supply one. Once `ink.toml` can, the value must be refused with a typed error at
  the config boundary.
- **`Budget` and `Failure` gained data.** `Failure::Memory` carries the size asked for as well as
  the limit; `Failure::Table` is new. The diagnostic codes are unchanged (the catalog is not
  touched here), so a refused table growth is still `plugin.call.trapped`, now with the numbers
  in its reason.
- **`ink.toml` is named twice** (`cli` and `engine`'s `name.rs`): a second copy of one fact.
- **The `data:` in the page CSP's `img-src`** is still there although placeholders are hashed
  files (0040); it is bucket 3 either way, but worth removing in its own change.
- **Fixed since the owner's list**: `Disk::BOUND` was already a public field; the work was to
  name it a limit and add `with_bound`. The `git` limits and the plugin budget's table and tick
  were the real hardcodes.
