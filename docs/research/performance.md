# Performance of the text path

What reading, interpreting, highlighting and rendering text costs, where the time goes, and
what was changed for it. Measured 2026-11-27 on the benches the repo carries; the image work,
the folder walk and the git source are measured on their own.

## How it is measured

Benches are divan (`libraries.md`), beside the code, on inputs `fixture::corpus` generates from
a seed, so every machine benches the same bytes:

- `cargo bench -p uses-ink-format-markdown`: `post` (one post of about 2,000 words with
  headings, lists, task lists, tables, footnotes, code in Rust, Python and TypeScript, math,
  callouts, definitions and allowed HTML), `flood` (20,000 short paragraphs, 1.6 MB),
  `headings` (5,000 headings of one text) and `folds` (5,000 `<details>` never closed).
- `cargo bench -p uses-ink-cli`: `interpret` and `render` (a blog of 500 such posts linking to
  one another), `page` (one of its pages through the theme), `flood` (the flood as a site of
  one page, interpreted and rendered), `notes` (a page of 20,000 footnotes through the theme)
  and `highlight` (each language's block, its grammar compiled).

The machine is shared, so wall time moved by a factor of three between runs. The figures below
are user-space instructions per iteration on one performance core (`taskset -c 2 perf stat -e
cpu_core/instructions/u`), the difference between a run of n + 1 iterations and a run of one, so
setup cancels out; they repeat to 0.01%. Wall times are divan's fastest at low load. Profiles
are `perf record --call-graph lbr`; DWARF unwinding fails through tree-sitter's C.

## As measured

| Bench | Before | After | Change |
|---|---:|---:|---|
| read `post` | 3.09 M | 3.10 M | none (0.35 ms) |
| read `flood` | 371 M | 372 M | none (48 ms) |
| read `headings` | 28,981 M | 95 M | 300 times fewer (8.2 s to 0.12 s) |
| read `folds` | 1,107 M | 70 M | 16 times fewer (363 ms to 49 ms) |
| site `interpret`, 500 posts | 1,578 M | 1,582 M | none (190 ms) |
| site `render`, 500 posts | 9,283 M | 8,943 M | 3.7% fewer (1.26 s) |
| site `page` | 17.7 M | 17.7 M | none (2.3 ms) |
| site `flood` | 624 M | 558 M | 11% fewer (75 ms) |
| site `notes` | 4,030 M | 291 M | 14 times fewer |
| `highlight` Rust, Python, TypeScript | 2.68, 1.48, 1.64 M | same | none (120 to 270 us) |
| `ink build`, 5,000 pages of 20 links and images each | 10,747 M | 8,068 M | 25% fewer |

The 500-post blog plus the flood builds through `ink build` in 12,607 M instructions before and
12,239 M after (about 2.4 s on one core), the deploy id unchanged (`bd6eb771...`), every file
byte for byte; the 5,000 linked pages keep theirs too.

## Where the time goes

**Rendering is highlighting.** On a post with eight code blocks, 79% of the render's cycles are
in `Grammars::highlight`, 73% inside arborium: tree-sitter parsing and running the highlight
query, and re-parsing Rust macro bodies its injection query asks for. Our sweep over the
captures (`Layers`) is about 6%. A grammar compiles once per process (decision 0039), so a
call's own setup is a parser and a query cursor: reusing them saved 0.5%, and merging the
sorted openings with the sorted closings instead of sorting both saved 0.6%. Both were
measured and left out.

**Interpreting is reading markdown.** `format_markdown::read` is 80% of interpreting the blog,
comrak's parse half of that; the engine's stages after it are about 10%, link resolution 6.5%
of it. The decisions, the journal and its index were not touched (decision 0042 will change
them).

**The flood is linear.** 20,000 paragraphs read in 48 ms and build as a site in 75 ms; twice
the paragraphs take 1.8 times as long. The 4 s the M1 safety review saw did not reproduce in a release build.

## What was changed

Each was quadratic in something a file controls, and each keeps output byte for byte:

- **Heading anchors**: the k-th heading of a text tried `intro-1` to `intro-k` again, a slug
  parse each. A taken name stays taken, so the count now goes on from where the last claim of
  that slug stopped.
- **Elements left open**: closing a run spilled its unclosed elements one at a time, each
  moving everything the inner ones had spilled into it. An element's content ends where the
  next one opened, so all of them are spilled at once, each node moved once, the diagnostics in
  the same order.
- **Footnote numbers**: the theme searched the footnote list for every reference; it now
  numbers them once per page.
- **Pages by id**: `Site::page` searched every page, and the theme asks it for every link; the
  site now holds an index from id to page, built in `Site::new` and not serialized.
- **The render's copy of the site**: `render` copied every page to attach the image pictures;
  a site whose assets get no picture is now rendered as interpreted.

## Ruled out or left

- The tag stack's close (`rposition` over open elements), footnote naming (`n log n`, its
  lowercase key and slug per reference), the summary and heading visits (once per page each),
  the link fold (in place) and the assemble stage (moves documents) showed nothing worth a
  change. Nothing on the native path round-trips through serde. The site is not copied per page.
- The theme writes each shell three times through `unspliced::Split` (count the mark, then
  each half). One pass with the tail held in a reused buffer saved 1% of a page and 1% of the
  flood, not enough to leave the documented shape.
- Math is 14% of the flood's render, two thirds of it in math-core's conversion; the
  allowlist's linear name tables are the rest.
- `image()` searches the site's assets for each image: 1% at 200 assets. Assets sort by file,
  so a binary search would do, once `Site::assets` says so.
- The frontmatter reader (3.7% of a post) and `Positions::new` (3%) are linear and small.
- Highlighting the same block twice is a host cache (by language and text hash) for `ink
  preview`'s rebuilds, not for one build; it is a decision of its own.
