# uses.ink: dependencies for the engine, builder, CLI and pen

Scope: crates for the engine (`uses-engine`, `uses-model`), the builder and `ink` binary,
and the `pen` crate. The dashboard framework and the Cloudflare control plane are out of
scope. Versions, release dates and download counts were read from the crates.io API on
2026-11-26. Advisories were read from a fresh clone of `rustsec/advisory-db` the same day.
Claims about APIs were checked against the published crate source (downloaded to
`~/code/strangers/crates-src/`) unless marked `[unverified]`.

Columns: **Recent** is crates.io `recent_downloads` (last 90 days). **Signal** is a short
maintenance note. **Fit** is how well it matches the architecture (pure engine, `Host`
for all I/O, deterministic output, typed `Decision` and `Diagnostic`). **Risk** is what
could hurt us.

Owner decision folded in (2026-11-26): reader themes are WebAssembly components in v1.
The builder therefore embeds wasmtime. Themes call the pen and text measurement as host
imports (`pen.stroke`, `text.measure`), so `pen` runs natively inside the builder and is
compiled to `wasm32-unknown-unknown` only for the dashboard in the browser.

## 1. Markdown

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `comrak` | 1.2.0 | 2026-11-23 | BSD-2-Clause | 1758 stars, 11 open issues, releases monthly, extensions being written up as specs | Arena AST with `Sourcepos` (line and column) on every node. Has GFM, footnotes, `front_matter_delimiter`, header ids, `math_dollars`, GitHub `alerts`, and Obsidian syntax: `ofm_callouts` (any type, fold state, title), `wikilinks_*` with `NodeWikiLink { url, embed }` so `![[x]]` is a node, `ofm_block_ids` (`^id`), `ofm_comments` (`%%..%%`), `highlight` (`==x==`) | 1.x keeps finding quadratic cases (1.2.0 fixed several, GHSA-7f8r-4rh5-vr5h). Default features pull clap and syntect: use `default-features = false` |
| `pulldown-cmark` | 0.13.6 | 2026-10-30 | MIT | Most used (79M recent), used by Zola and mdBook | Event stream with byte ranges via `into_offset_iter`. Has wikilinks (`LinkType::WikiLink`), math, YAML metadata blocks, heading attributes, GFM alerts | Alerts are only the 5 GitHub kinds (`BlockQuoteKind`), so Obsidian callouts with custom types, titles and folding need our own pass. No block ids, no `%%` comments. Two 2026 advisories on wikilinks: RUSTSEC-2026-0414 (exponential events) and RUSTSEC-2026-0433 (quadratic nested image wikilinks) |
| `markdown` (markdown-rs) | 1.0.0 | 2025-04-23 | MIT | Repo last pushed 2025-04, 107 open issues | mdast with positions, GFM, math, frontmatter, MDX | No wikilinks or callouts, no extension point for new syntax. Stalled |

Path to our typed `Document`: with comrak, parse once with `sourcepos` on, then one
recursive walk from `AstNode` into our `Document` enum, turning `Sourcepos` into our
`Span` (we hold the source, so line and column map to byte offsets once per file). Link
resolution happens after, against the Site, so the tree only stores raw targets until the
`links` inferrer replaces them with `PageRef` or `Anchor`. Header ids: compute our own
anchors in the walk (one rule for headings and for `[[Note#Heading]]`), do not use
comrak's `header_id_prefix`, since Obsidian heading links must match our slug rule.

Pick: **`comrak`** with `default-features = false`. It is the only parser that already
produces nodes for every Obsidian construct we listed, with spans.

## 2. YAML frontmatter

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `serde_yaml` | 0.9.34+deprecated | 2024-03-25 | MIT OR Apache-2.0 | Deprecated by its author | Works | No fixes coming. Zola still uses it |
| `serde_yml` | 0.0.13 | 2026-05-27 | MIT OR Apache-2.0 | RUSTSEC-2025-0068: unsound and unmaintained; its `libyml` base has RUSTSEC-2025-0067 | None | Do not use |
| `serde_norway` | 0.9.42 | 2024-12-21 | MIT OR Apache-2.0 | Fork of serde_yaml, quiet for a year | Drop-in | Same unsafe-libyaml port, no spans |
| `serde_yaml_ng` | 0.10.0 | 2024-05-26 | MIT OR Apache-2.0 | Fork, quiet since 2024 | Drop-in | Same as above |
| `serde-saphyr` | 2.2.0 (3.0.0-rc.1 out) | 2026-11-18 | MIT OR Apache-2.0 | 256 stars, 2 open issues, very active, 18M recent | Pure Rust, panic-free, `Spanned<T>` with `Location { line, column, span }` for both where a value is referenced and where an anchor defined it, budget limits against alias bombs. Optional miette and annotate-snippets integration | 3.0 is in RC: expect one breaking bump soon. Uses its own `granit-parser` fork of the saphyr parser |
| `saphyr` | 0.4.1 | 2026-11-25 | MIT OR Apache-2.0 | Active | YAML 1.2 tree with marked spans, no serde | We would write the mapping into typed frontmatter ourselves |
| `yaml-rust2` | 0.13.2 | 2026-11-21 | MIT OR Apache-2.0 | Active, 25M recent | Tree API, no spans in the public value type `[unverified]` | No spans for diagnostics |
| `marked-yaml` | 0.8.0 | 2025-05-26 | MIT | Small | Spans on every node | Small audience |

Pick: **`serde-saphyr`**, deserializing frontmatter into a struct of `Spanned<T>` fields so
`Evidence::Frontmatter { span }` is real. Pin 2.x until 3.0 is final.

## 3. TOML for `ink.toml`

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `toml` | 1.2.3 (spec 1.1.0) | 2026-11-25 | MIT OR Apache-2.0 | Cargo's parser, 342M recent | serde, works with `serde_spanned::Spanned<T>` for `Config { span }` | None notable |
| `toml_edit` | 0.25.21 | 2026-11-25 | MIT OR Apache-2.0 | Same repo as `toml` | Format-preserving edits for `ink explain --pin` and dashboard writes | Still 0.x API |
| `serde_spanned` | 1.1.3 | 2026-11-06 | MIT OR Apache-2.0 | Same repo | Byte ranges for any field | None |
| `toml-span` | 0.7.2 | 2026-10-02 | MIT OR Apache-2.0 | Embark, active | Spans on every value, no serde | Second parser in the tree for no gain |
| `serde_ignored` | 0.1.14 | 2025-09-14 | MIT OR Apache-2.0 | Stable | Reports the path of every unknown key, so "unknown key, did you mean" works | None |
| `strsim` | 0.11.1 | 2024-04-02 | MIT | Done, not dead | Suggestions for unknown keys | None |
| `schemars` | 1.2.3 | 2026-10-12 | MIT | 272M recent | JSON Schema from the same config structs; Taplo reads it via a `#:schema` line or a catalog entry | Generated schema needs a CI check that it is committed and current |

Pick: **`toml` + `serde_spanned` + `serde_ignored` for reading, `toml_edit` for writing,
`schemars` for the schema.** One parser family, spans on every value we report.

## 4. Git

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `gix` | 0.91.0 | 2026-11-22 | MIT OR Apache-2.0 | 12k stars, monthly releases, jj and cargo listed as downstream QA targets in its manifest | Pure Rust. Fetch and clone over https with `blocking-http-transport-reqwest-rust-tls`. `remote::fetch::Shallow` has `DepthAtRemote(n)`, `Deepen(n)`, `Since { cutoff }`, `Exclude`, `undo()`, so shallow then deepen is supported. Rev walk and tree diff for per-file dates; a `blame` feature (`gix-blame` 0.21) exists | Still 0.x, breaking every month. No push needed, so the missing parts do not hit us. Partial clone (`--filter=blob:none`) is only a protocol argument, the object store cannot lazily fetch missing blobs `[unverified]`. Several gix-* sub-crate advisories over time (10 sub-crates have entries) |
| `git2` | 0.21.0 | 2026-10-06 | MIT OR Apache-2.0 | Mature libgit2 binding | Everything works, including partial clone through libgit2 `[unverified]` | C dependency (libgit2, plus OpenSSL or system TLS) in the container image. Three 2026 "unsound" advisories (RUSTSEC-2026-0008, -0183, -0184) |

Per-file first and last commit for a few thousand files: one first-parent rev walk from
HEAD, diff each commit tree against its parent with rename tracking off, record the
first time a path is seen (updated) and keep overwriting the last time (created). That is
one pass over history and one tree diff per commit, which gix does from the
commit-graph and pack without checkout. When the walk hits the shallow boundary, the
remaining paths get `Known::NotAfter` or `Unknown { BeyondDepth }`, and the builder can
`Deepen` and resume. No blame is needed for dates.

Pick: **`gix`** with `default-features = false` and only the fetch, revision and
blob-diff features. Pure Rust keeps the container image small and the build hermetic.

## 5. Hashing and HTML tools

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `blake3` | 1.8.7 | 2026-08-20 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | Reference implementation | Content addressing, render-unit keys. Builds for wasm with the portable code path | None |
| `ammonia` | 4.2.5 | 2026-11-13 | MIT OR Apache-2.0 | Active | Whitelist sanitizer over html5ever | Four advisories in 2026 alone (RUSTSEC-2026-0193, -0213, -0373, -0533), mostly SVG and MathML mutation XSS. Must be upgraded the day an advisory lands |
| `lol_html` | 3.0.3 | 2026-10-28 | BSD-3-Clause | Cloudflare, active | Streaming rewriter with CSS selectors | We render from a typed tree, so we rarely need it. Useful for post-processing Typst HTML output |
| `minify-html` | 0.18.1 | 2025-10-25 | MIT | One maintainer, repo pushed 2025-11, 78 open issues; Zola uses it | Deterministic minifier | A year without a release. Our renderer can emit compact HTML itself |

Raw HTML in markdown is the real XSS surface on `<handle>.uses.ink`. Plan: parse raw HTML
blocks into our `Document` through an allowlist of our own and drop the rest with a
diagnostic, so the reader page never contains author HTML we did not model. Keep
`ammonia` as the fallback for an explicit "trusted HTML" escape hatch, if one is ever
added.

Picks: **`blake3`** yes. **`ammonia`** only behind an escape hatch, with `cargo deny`
watching advisories. **`lol_html`** only for Typst output if needed. **No `minify-html`
in v1**: emit compact HTML directly and compress at the edge.

## 6. Build-time syntax highlighting (classed spans)

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `giallo` | 0.6.0 | 2026-11-04 | **EUPL-1.2** | Zola's own highlighter (Zola uses 0.5), by Zola's author | TextMate grammars and themes from Shiki, 220+ grammars and about 60 themes in a 1.4 MB zstd dump, same output as VSCode. `HtmlRenderer { css_class_prefix }` plus `Registry::generate_css(theme, prefix)` give classed spans and a CSS file per theme. 0.6 uses `fancy-regex` (pure Rust), the README still mentions an oniguruma fork | EUPL-1.2 is a copyleft licence: fine for a hosted service and for linking `[unverified legal reading]`, but it must be compatible with whatever licence uses.ink ships under. `zstd` is a C dependency |
| `arborium` | 2.18.2 | 2026-08-28 | MIT OR Apache-2.0 (grammars keep their own licences) | 512 stars, bearcove (fasterthanlime), sponsors include Zed | Tree-sitter, about 70 permissive grammars by default, `HtmlFormat::ClassNames` or `ClassNamesWithPrefix`. Has a wasm build | Each grammar is compiled C; binary and build time grow with languages (tens of MB with all of them `[unverified]`). README says it is co-developed with LLMs |
| `lumis` (was `autumnus`) | 0.19.7 | 2026-11-23 | MIT | Active, 243 stars | Tree-sitter, 110+ languages, Neovim themes, HTML inline or linked (classed) | "Load parsers on demand, verified and cached" means a runtime download path we must keep off for determinism |
| `syntect` | 5.3.0 | 2025-09-27 | MIT | 145 open issues, slow releases; typst depends on it | Sublime grammars, `ClassedHTMLGenerator` | Older grammar set (extend with `two-face` 0.5.2), grammar quality behind Shiki |
| `tree-sitter-highlight` | 0.27.2 | 2026-11-07 | MIT | Upstream | Raw highlighter, bring your own grammars and queries | All the grammar plumbing is on us |
| `inkjet` | 0.11.1 | 2024-09-15 | MIT OR Apache-2.0 | Repo archived | None | Dead |

Pick: **`giallo`** if the owner accepts EUPL-1.2 in the dependency tree (best grammar
coverage, pure Rust regex, classed CSS out of the box). Otherwise **`arborium`** with a
curated language list.

## 7. Typst

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `typst` | 0.15.1 | 2026-07-17 | Apache-2.0 | Active, minors break APIs | `World` trait: we implement it over our `Host` (files from the Snapshot, `today()` from the build fact, fonts from our font set) | Heavy tree. typst-library 0.15.1 depends on `rustybuzz` 0.20 and `ttf-parser` 0.25 (both flagged unmaintained in 2026), `usvg` 0.47 (we would use 0.50), `syntect`, `hayagriva`, `image`. Expect duplicate font and SVG crates |
| `typst-html` | 0.15.1 | 2026-07-17 | Apache-2.0 | Same | HTML export with a `mathml.rs` module (1346 lines): equations become `<math>` (`display="block"` for block math). `html_mathml_body` is public | Still gated behind `Feature::Html` in `typst-library` (opt-in, experimental). A new `Feature::Bundle` exists for multi-file output |
| `typst-kit` | 0.15.1 | 2026-07-17 | Apache-2.0 | Same | Font search, embedded fonts, package download | Package download and system font scan break determinism; use only the embedded-font parts, or none |
| `typst-as-lib` | 0.18.0 | 2026-11-17 | MIT | 128 stars, active | Ready-made `World` with file resolvers and a `typst-html` feature | Adds a layer we would replace anyway, since our files come from `Host` |
| `math-core` | 0.9.0 | 2026-10-12 | MIT | Active (pushed 2026-11-23), 49 stars | LaTeX to MathML Core, for `$..$` in markdown | Young; LaTeX coverage gaps will show up as diagnostics |
| `pulldown-latex` | 0.8.0 | 2026-07-28 | MIT | 74 stars | LaTeX to MathML | Same class, less active |

Markdown math is LaTeX syntax (Obsidian uses MathJax), not Typst syntax. Rendering it
through Typst would need a LaTeX-to-Typst translation step. Use a LaTeX-to-MathML crate
for markdown, and `typst-html` only for `.typ` files.

Pick: **own `World` over `Host` on `typst` + `typst-html`, later, behind a feature;
`math-core` for markdown math now.** Skip `typst-as-lib` and the networked parts of
`typst-kit`.

## 8. Images

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `image` | 0.25.10 | 2026-03-10 | MIT OR Apache-2.0 | Standard | Decode JPEG (zune-jpeg), PNG, GIF, WebP, AVIF (dav1d, optional) | None notable |
| `fast_image_resize` | 6.2.0 | 2026-11-20 | MIT OR Apache-2.0 | Active, Zola uses it | SIMD resize with `image` integration | None |
| `ravif` | 0.13.0 | 2026-01-19 | BSD-3-Clause | Kornel | Pure Rust AVIF encode via rav1e | Slow encodes; budget CPU per build or encode once per blob hash |
| `image-webp` | 0.2.4 | 2025-08-27 | MIT OR Apache-2.0 | Used by `image` | Decoder complete | **Encoder is lossless only** (README and `encoder.rs`) |
| `webp` | 0.3.1 | 2025-08-29 | MIT OR Apache-2.0 | Wraps libwebp (C), Zola uses it | Lossy WebP | C dependency |
| `jpeg-encoder` | 0.7.1 | 2026-07-27 | (MIT OR Apache-2.0) AND IJG | Used by `image` | Baseline and progressive JPEG | None |
| `zune-image` | 0.5.0 | 2026-01-24 | MIT OR Apache-2.0 OR Zlib | Small audience | Alternative pipeline | Not needed; `zune-jpeg` already comes in via `image` |
| `imagesize` | 0.15.0 | 2026-07-09 | MIT | Stable, used by usvg | Width and height from headers, for `width`/`height` attributes without decoding | None |
| `img-parts` | 0.4.2 | 2026-11-03 | MIT OR Apache-2.0 | Active | Drop EXIF/XMP segments from JPEG, PNG, WebP without re-encoding | No AVIF/HEIC |
| `little_exif` | 0.6.23 | 2026-01-13 | MIT OR Apache-2.0 | One maintainer | Read and write EXIF incl. AVIF, HEIC, JXL | Young |
| `kamadak-exif` | 0.6.2 | 2026-11-05 | BSD-2-Clause | Stable | Read orientation before stripping | Read only |
| `thumbhash` | 0.1.0 | 2023-03-22 | MIT | Reference port by evanw, 332 lines | Tiny placeholders | No release since 2023; small enough to vendor |

Re-encoding always strips metadata, so EXIF stripping only matters for originals we
pass through. Read orientation first (`kamadak-exif`), apply it, then re-encode.

Pick: **`image` + `fast_image_resize` + `ravif` (AVIF) + `jpeg-encoder` (fallback)**,
`imagesize` for dimensions, `img-parts` for passing originals through clean, `thumbhash`
vendored. Skip lossy WebP unless the owner wants libwebp (C) in the image.

## 9. Fonts and text measuring

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `skrifa` | 0.50.0 | 2026-11-10 | MIT OR Apache-2.0 | Google Fonts "fontations", the RustSec-named replacement for ttf-parser | Metrics, advances, outlines (pen can trace glyphs) | Version skew: cosmic-text pins 0.40, subsetter 0.42, usvg 0.49, so several copies may coexist |
| `harfrust` | 0.16.1 | 2026-11-19 | MIT | HarfBuzz project, the RustSec-named replacement for rustybuzz | Shaping for exact widths (kerning, ligatures) | None |
| `rustybuzz` | 0.20.1 | 2024-11-12 | MIT | **RUSTSEC-2026-0206: unmaintained** | Was the standard | Still pulled in by typst 0.15 |
| `ttf-parser` | 0.25.1 | 2024-11-29 | MIT OR Apache-2.0 | **RUSTSEC-2026-0192: unmaintained** | Was the standard | Same |
| `parley` | 0.13.0 | 2026-11-12 | Apache-2.0 OR MIT | Linebender, very active | Full layout (line breaking, bidi) on fontique + harfrust | More than heading and link widths need |
| `cosmic-text` | 0.19.0 | 2026-04-22 | MIT OR Apache-2.0 | System76 | Layout plus a font database | Pins old harfrust and skrifa |
| `swash` | 0.2.10 | 2026-07-17 | Apache-2.0 OR MIT | Slow | Shaping and scaling | Overlaps with harfrust + skrifa |
| `subsetter` | 0.2.6 | 2026-06-04 | MIT OR Apache-2.0 | Typst's subsetter | Subset TrueType and CFF by glyph set | Outputs OpenType; WOFF2 is a separate step |
| `ttf2woff2` | 0.14.1 | 2026-10-18 | MIT OR Apache-2.0 | 13 stars, one maintainer, created 2026-01 | Pure Rust WOFF2 encode (brotli crate), single-threaded mode documented as deterministic | Small project; glyf transform off by default (slightly larger files) |
| `woofwoof` | 1.0.2 | 2026-01-02 | MIT OR Apache-2.0 | fasterthanlime | Google's woff2 C++ for transforms plus Rust brotli | C++ in the build |
| `woff2` | 0.3.0 | 2022-05-05 | Apache-2.0 | Dead | None | Dead |

Measuring for per-element pen marks: build time already knows the theme font and the
text, so `harfrust` gives the advance of a heading or link at a given size, and
`skrifa` gives ascent and descent. Line wrapping is not known at build time (it depends on
the reader's viewport), so build-time marks fit single-line elements (headings, nav
links, buttons) and multi-line underlines stay CSS. With themes as wasm components,
this becomes the host import `text.measure(font, size, text) -> metrics`.

Pick: **`harfrust` + `skrifa` for `text.measure`, `subsetter` + `ttf2woff2` for web
fonts.** Add `parley` only if a theme needs wrapped layout.

## 10. Pen

What the JS does today (`pen-core.js`): mulberry32 RNG, FNV-1a hash over UTF-16 code
units (`charCodeAt`), straight segments with overshoot, `humanise` (two low-frequency
sines, bow, tremor, pressure curve), perfect-freehand `getStroke` with fixed options,
outline to a quadratic SVG path, and `Bucket.shape` (jittered, smoothed polygon fill).
`pen.js` adds DOM layers and draw-in animation; the only DOM geometry it needs is
`getTotalLength` and `getPointAtLength` for sampling a path.

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `perfect_freehand` | 0.1.1 | 2026-01-27 | MIT | 4 stars, one author, zero deps, 801 lines | Full option set: `thinning`, `smoothing`, `streamline`, `simulate_pressure`, `start`/`end` with `cap` and `taper`, `last`, `closed` | Tiny project. Options are all `Option<_>`, easing is `fn(f64) -> f64` |
| `freedraw` | 1.0.4 | 2025-05-24 | MIT `[unverified]` | 5 stars | Same port, 2312 lines with demo | Same |
| `kurbo` | 0.13.1 | 2026-05-13 | Apache-2.0 OR MIT | Linebender, very active | `BezPath::from_svg`/`to_svg`, `ParamCurveArclen::arclen` and `inv_arclen` (replaces `getTotalLength`/`getPointAtLength`), flattening, `libm` feature for no_std and wasm | None |
| `lyon` | 1.0.19 | 2026-03-08 | MIT OR Apache-2.0 | Stable | GPU tessellation | Not needed |
| `usvg` / `resvg` | 0.50.0 | 2026-11-11 | Apache-2.0 OR MIT | Linebender | SVG to PNG for OG images and favicons; text via `harfrust` 0.15 + `skrifa` 0.49 + `fontdb` | typst 0.15 pins usvg 0.47, so two versions if we use both |
| `tiny-skia` | 0.12.0 | 2026-02-02 | BSD-3-Clause | Linebender | Rasterize our own paths directly, no SVG round trip | None |
| `ico` | 0.6.0 | 2026-10-10 | MIT | Stable | `favicon.ico` from PNGs | Only if we ship `.ico` at all |
| `libm` | 0.2.16 | 2026-03-17 | MIT | rust-lang | Same `sin`/`cos`/`hypot` bits on every target | None |
| `rand_pcg` | 0.10.3 | 2026-11-18 | MIT OR Apache-2.0 | rand project | Deterministic RNG | Not needed: mulberry32 is 8 lines and must match the JS sequence exactly |
| `wasm-bindgen` | 0.2.130 | 2026-10-30 | MIT OR Apache-2.0 | Standard | Dashboard build | CLI version must match the crate exactly (the build failed with CLI 0.2.129 and crate 0.2.130) |

Spike (scratchpad, not in the repo): mulberry32 + FNV + a reduced `humanise` +
`perfect_freehand::get_stroke` + `kurbo` path sampling + one `#[wasm_bindgen]` export.
Release build with `opt-level = "z"`, LTO, `panic = "abort"`, no `wasm-opt`:
**115 KB wasm, 57 KB gzipped**. The same four strokes rendered natively and in Node from
the wasm build hashed identically (`58a9bbca...`). That is one sample, not a proof: the
port calls `f64::sin`/`atan2` from std, which uses the platform libm natively and Rust's
libm port on wasm. For guaranteed bit-equality, our port must route all transcendental
math through `libm` and format numbers with our own fixed-precision writer.

Shape of the crate: `pen` is a pure `no_std + alloc` library with no I/O. Inputs are
plain geometry and a seed string; outputs are path data and fills as typed values, with
SVG text as one serializer and `tiny-skia` paths as another. The builder exposes it to
themes as host imports (`pen.stroke`, `pen.fill`), the CLI uses it for logo, marks and
OG images, and a thin `pen-wasm` crate wraps it with `wasm-bindgen` for the dashboard.
The DOM parts of `pen.js` (layers, draw-in) stay as small dashboard JS that calls the wasm.

Pick: **own port of perfect-freehand inside `pen`** (about 400 lines, MIT upstream, the
Rust ports are reference material, not dependencies), **`kurbo`** for path parsing and
arc-length sampling, **`libm`**, **own mulberry32 and FNV**, **`tiny-skia`** for
raster, **`resvg`** only where we need to rasterize SVG with text (OG images).

## 11. Feeds and site files

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `atom_syndication` | 0.12.10 | 2026-07-30 | MIT/Apache-2.0 | rust-syndication, slow but alive | Typed Atom builder over quick-xml | None |
| `rss` | 2.1.2 | 2026-09-10 | MIT/Apache-2.0 | Same org | Typed RSS 2.0 | None |
| `sitemap` | 0.4.1 | 2020-11-03 | MIT | Dead | None | Dead |
| `sitemap-rs` | 0.7.1 | 2026-11-13 | MIT | 34 stars | Sitemap with image and news extensions | Small project for a 20-line file |
| `quick-xml` | 0.42.0 | 2026-08-22 | MIT | Standard | Writer for sitemap and OPML | None |
| `slug` | 0.1.6 | 2024-08-15 | MIT/Apache-2.0 | Done | `deunicode` + lowercase + hyphens | Transliterates CJK into pinyin, which may not be what an author wants |
| `deunicode` | 1.6.2 | 2025-04-27 | BSD-3-Clause | Stable | Transliteration | Same |
| `unicode-segmentation` | 1.13.3 | 2026-06-01 | MIT OR Apache-2.0 | Standard | Grapheme-safe summaries and truncation | None |
| `unicode-normalization` | 0.1.26 | 2026-10-16 | (MIT OR Apache-2.0) AND Unicode-3.0 | Standard | NFC for paths and link targets | None |

Pick: **`atom_syndication` + `rss`**, sitemap written with **`quick-xml`**, **own slug
rule** (NFC, then lowercase, keep letters of any script, hyphens) with `deunicode` as an
opt-in `ink.toml` choice, **`unicode-segmentation`** and **`unicode-normalization`**.

## 12. Static search

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `pagefind` | 1.6.0 | 2026-11-01 | MIT | 5.6k stars, active | `pagefind::api::PagefindIndex`: `add_html_file`, `add_custom_record`, `build_indexes`, `get_files` returning in-memory `SyntheticFile`s, so no disk I/O is required | Async (tokio). Default feature `serve` pulls actix-web: use `default-features = false`. Reader side needs `pagefind.js` plus a wasm search module, which breaks "reader pages are static HTML with no JS". Byte-identical output across runs `[unverified]` |

Pick: **`pagefind` as a library, opt-in per site, JS only on the search page.** Run
`ink build --check` against a fixture with search on before relying on its determinism.

## 13. CLI and diagnostics

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `clap` | 4.6.10 | 2026-11-24 | MIT OR Apache-2.0 | Standard | Derive API, `clap_complete` | None |
| `annotate-snippets` | 0.12.18 | 2026-11-11 | MIT OR Apache-2.0 | rust-lang, what rustc and cargo render with now, very active | Renderer only: takes source, spans, labels. Our `Diagnostic` stays our type | Less "fancy" than miette by default |
| `miette` | 7.6.0 | 2025-04-27 | Apache-2.0 | No release in 19 months, 115 open issues | `Diagnostic` trait and derive | Wants to own the diagnostic type, which competes with our catalog |
| `ariadne` | 0.6.0 | 2025-10-28 | MIT | One maintainer | Pretty multi-span output | Slow release cadence |
| `codespan-reporting` | 0.13.1 | 2025-10-22 | Apache-2.0 | Stable | Renderer | Fine, less active than annotate-snippets |
| `tracing` + `tracing-subscriber` | 0.1.44 / 0.3.23 | 2025-12-18 / 2026-03-13 | MIT | Standard | Build events and spans; JSON layer for the container | None |
| `notify` | 9.0.0 | 2026-11-21 | CC0-1.0 | Major release five days old | `ink watch` | Fresh major: expect a point release or two |
| `notify-debouncer-full` | 0.8.0 | 2026-11-21 | MIT OR Apache-2.0 | Paired with notify 9 | Rename-aware debouncing | Same |
| `indicatif` | 0.18.6 | 2026-07-01 | MIT | Standard | Progress bars for fetch and render | Draw only when stderr is a terminal; plain event lines otherwise |

Pick: **`clap`, `annotate-snippets`, `tracing`, `notify` + `notify-debouncer-full`,
`indicatif`.** Diagnostics stay our own typed values; `annotate-snippets` only draws them.

## 14. WIT and the plugin host

Changed by the owner decision: themes are components in v1, so wasmtime is a builder
dependency now.

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `wasmtime` | 51.0.0 | 2026-11-20 | Apache-2.0 WITH LLVM-exception | Monthly majors; 36.x still patched (36.0.23 on 2026-11-19), so an LTS line exists | `component::bindgen!` generates host traits for imports and typed wrappers for exports. Default features include `component-model`, `component-model-async`, `pooling-allocator`, `cranelift`, `cache`. `Engine::precompile_component`, `Component::deserialize_file`, `Engine::precompile_compatibility_hash`, `Engine::detect_precompiled`. `Config::consume_fuel`, `Config::epoch_interruption`, `ResourceLimiter` for memory, `cranelift_nan_canonicalization` and `relaxed_simd_deterministic` for determinism | Large compile and binary (cranelift). A major every month means bindgen output and config APIs move; track the LTS line |
| `wit-bindgen` | 0.63.0 | 2026-10-14 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Bytecode Alliance | Guest side for themes (`wasm32-wasip2` or `wasm32-unknown-unknown` + component adapter) | Generated imports compile on native but are `unreachable!()` stubs there (`#[cfg(not(target_arch = "wasm32"))]`), so guest bindings cannot run natively |
| `wit-parser` | 0.263.0 | 2026-10-30 | same as wit-bindgen | Bytecode Alliance | Pure Rust parser and resolver; deps are small (anyhow, id-arena, indexmap, semver, wasmparser) | We own any codegen we write on it |

Status as checked in wasmtime's `docs/stability-tiers.md` today: `component-model` and
`component-model-async` are both **Tier 1**. The old `Config::async_support` is now a
deprecated no-op; async is governed by the crate feature and
`Config::wasm_component_model_async`. Themes do not need async: a theme is a pure
function from the Site slice to files, so call it with sync bindings and keep the
`async` feature off unless the host must await inside an import.

Recommended setup for themes:

- **Bindings**: `wasmtime::component::bindgen!` on the host from `wit/uses-theme.wit`;
  `wit-bindgen` in theme crates. The same WIT types generate both, so `uses-model` keeps
  plain Rust types and converts at the boundary (or uses bindgen's `with` option to map
  WIT records onto `uses-model` types).
- **Native first-party plugins** (sources, interpreters) that are not wasm: still no
  off-the-shelf generator gives plain Rust traits for both imports and exports without
  wasmtime. Since wasmtime is now in the tree anyway, the cheap path is to reuse the
  host-side `bindgen!` traits: the generated `Host` traits for imports are plain Rust
  traits, and a native plugin can implement the export side as the same Rust trait we
  hand-write next to the WIT. The fallback is a small `build.rs` codegen on `wit-parser`
  that emits records, variants and two traits; a few hundred lines `[unverified estimate]`.
- **Precompile**: compile each theme to `.cwasm` once with `Engine::precompile_component`,
  store it as a blob keyed by `blake3(theme wasm, precompile_compatibility_hash)`, and
  load with `Component::deserialize`. Only deserialize bytes we produced (the call is
  `unsafe` for that reason).
- **Limits**: `consume_fuel` for a deterministic CPU budget (same fuel, same failure, so
  the diagnostic is reproducible), `epoch_interruption` as a wall-clock backstop,
  `ResourceLimiter` (or `StoreLimitsBuilder`) for memory, pooling allocator for many
  small instantiations per build.
- **Determinism**: link no WASI clocks or random into themes; time comes only from facts.
  Turn on `cranelift_nan_canonicalization` and `relaxed_simd_deterministic`.
- **Pen and text as host imports**: `pen.stroke`, `pen.fill`, `text.measure` are
  implemented natively in the builder, so themes stay small and every theme draws the
  same pen.

Pick: **`wasmtime` (component model, sync, fuel + epoch + pooling, precompiled `.cwasm`)
on the host, `wit-bindgen` in themes, `wit-parser` only if native plugins need generated
traits.** Pin to a wasmtime LTS line once one covers our needs.

## 15. Dates

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `jiff` | 1.0.3 | 2026-11-24 | Unlicense OR MIT | 1.0.0 released 2026-11-15, BurntSushi | `Timestamp`, civil dates, `strftime` with English names, embedded tz database option (`tzdb-bundle-always`) for determinism | Fresh 1.0 |
| `time` | 0.3.55 | 2026-08-01 | MIT OR Apache-2.0 | Standard, used by typst and Zola | Works | RUSTSEC-2026-0009 (Rfc2822 formatting); 0.x forever |
| `chrono` | 0.4.45 | 2026-06-04 | MIT OR Apache-2.0 | **RUSTSEC-2026-0543: unmaintained** (2026-09-14) | None | Avoid; typst-kit has it as an optional dep |
| `icu` / `icu_datetime` | 2.4.0 | 2026-10-22 | Unicode-3.0 | Unicode Consortium | Real locale formatting | Data size and complexity; not needed for a fixed locale table |
| `jiff-icu` | 1.0.0 | 2026-11-18 | Unlicense OR MIT | Same author | Bridge if ICU is ever needed | None |

Pick: **`jiff`** with a bundled tz database, and our own fixed locale table (month and
weekday names for the handful of site languages we support). ICU4X is not worth it
until we support many locales.

## 16. Testing

| Crate | Version | Last release | License | Signal | Fit | Risk |
|---|---|---|---|---|---|---|
| `insta` | 1.49.0 | 2026-10-03 | Apache-2.0 | Standard, Zola uses it | Snapshots of `Document`, decisions, diagnostics, rendered HTML | None |
| `proptest` | 1.12.0 | 2026-10-10 | MIT OR Apache-2.0 | Standard | Parser never panics, link resolution invariants, pen output stays finite | None |
| `similar` | 3.2.1 | 2026-10-24 | Apache-2.0 | Used by insta | Text diffs for `ink build --check` reports | None |
| `html5ever` / `scraper` | 0.40.1 / 0.29.0 | 2026-09-14 / 2026-10-21 | MIT OR Apache-2.0 / ISC | Servo | Parse output HTML to assert structure; serialize to a canonical form before snapshotting | None |

HTML diffing: no maintained crate does semantic HTML diff well. Snapshot a canonical
pretty-print (parse with html5ever, sort attributes, indent) with insta, and assert
targeted facts with `scraper` selectors.

Pick: **`insta` + `proptest` + `similar`, `scraper` for structural asserts.** Add
`cargo-deny` in CI for advisories and licences (EUPL and the grammar licences above).

## 17. What not to hand-roll

What Zola 0.23.6 (2026-09) uses: pulldown-cmark, giallo, tera, minify-html, image +
fast_image_resize + webp, serde_yaml, toml, time and jiff, globset, walkdir, rayon,
relative-path, slug, unicode-segmentation, ammonia, elasticlunr-rs, notify-debouncer-full,
insta. mdBook 0.5.4 uses pulldown-cmark, handlebars, elasticlunr-rs, ignore, notify,
toml 1.x, tracing.

| Crate | Version | Last release | License | Use |
|---|---|---|---|---|
| `globset` / `ignore` | 0.4.22 / 0.4.35 | 2026-11-22 | Unlicense OR MIT | `ignore` decision rules and gitignore semantics over Snapshot paths (`globset` alone, since the engine does not walk a disk) |
| `relative-path` | 2.0.3 | 2026-10-21 | MIT OR Apache-2.0 | `ContentPath` as a platform-independent relative path |
| `url` / `percent-encoding` | 2.5.9 / 2.3.2 | 2026-11-20 / 2025-08-21 | MIT OR Apache-2.0 | Link classification and route encoding |
| `mime_guess` | 2.0.5 | 2024-06-29 | MIT | Content types in the manifest (or a fixed table we own, for determinism) |
| `v_htmlescape` / `html-escape` | 1.0.0 / 0.3.1 | 2026-11 | MIT | Escaping in the HTML writer |
| `rayon` | 1.12.0 | 2026-04-14 | MIT OR Apache-2.0 | Parallel render units in the builder (the engine stays sequential and pure; the builder fans out) |
| `unicode-normalization` | 0.1.26 | 2026-10-16 | (MIT OR Apache-2.0) AND Unicode-3.0 | macOS vaults store NFD filenames; normalize paths and wikilink targets to NFC before resolution |
| `brotli` | 9.1.3 | 2026-11-15 | BSD-3-Clause AND MIT | Precompressed `.br` blobs if the edge serves them |

Templates for themes are no longer an engine question (themes are components), so
tera, minijinja and askama are left to the theme SDK.

## Pick list

| Need | Pick | Why |
|---|---|---|
| Markdown | `comrak` 1.2 (no default features) | Only parser with nodes for wikilinks, embeds, callouts, block ids and comments, with spans |
| Frontmatter | `serde-saphyr` 2.2 | Maintained, pure Rust, `Spanned<T>` for evidence spans |
| `ink.toml` | `toml` 1.2 + `serde_spanned` + `serde_ignored` + `toml_edit` 0.25 + `schemars` 1.2 | Spans, unknown-key diagnostics, format-preserving pins, Taplo schema |
| Git | `gix` 0.91 (minimal features) | Pure Rust, shallow plus `Deepen`, rev walk and tree diff for dates |
| Hashing | `blake3` 1.8 | Reference implementation |
| HTML safety | own allowlist into `Document`; `ammonia` 4.2 only for an escape hatch | Smallest XSS surface |
| HTML rewrite / minify | none in v1 (`lol_html` 3.0 if Typst output needs it) | We emit the HTML ourselves |
| Highlighting | `giallo` 0.6 (or `arborium` 2.18 if EUPL is a no) | VSCode-grade grammars, classed CSS, pure Rust regex |
| Typst | `typst` + `typst-html` 0.15 with our own `World`, later | MathML output; deterministic only with our `World` |
| Markdown math | `math-core` 0.9 | LaTeX to MathML Core, no JS for readers |
| Images | `image` 0.25 + `fast_image_resize` 6.2 + `ravif` 0.13 + `jpeg-encoder` 0.7 + `imagesize` 0.15 + `img-parts` 0.4 + vendored thumbhash | Pure Rust AVIF and JPEG, no C |
| Fonts | `harfrust` 0.16 + `skrifa` 0.50 + `subsetter` 0.2 + `ttf2woff2` 0.14 | Maintained successors of rustybuzz and ttf-parser, pure Rust WOFF2 |
| Pen | own `pen` crate: own perfect-freehand port, `kurbo` 0.13, `libm`, own mulberry32/FNV, `tiny-skia` 0.12, `resvg` 0.50 for OG images, `wasm-bindgen` for the dashboard | One deterministic core; 57 KB gzipped wasm in the spike |
| Feeds and site files | `atom_syndication` 0.12 + `rss` 2.1 + `quick-xml` 0.42; own slug rule; `unicode-segmentation`, `unicode-normalization` | Typed feeds, no dead sitemap crate |
| Search | `pagefind` 1.6 library, opt-in | In-memory API; JS only on the search page |
| CLI | `clap` 4.6, `annotate-snippets` 0.12, `tracing`, `notify` 9 + `notify-debouncer-full` 0.8, `indicatif` 0.18 | Our Diagnostic stays ours; rustc's renderer |
| Plugins and themes | `wasmtime` 51 (component, sync, fuel, epoch, pooling, `.cwasm`), `wit-bindgen` 0.63 for guests, `wit-parser` 0.263 if needed | Component model and async are Tier 1; deterministic limits |
| Dates | `jiff` 1.0 + own locale table | chrono is unmaintained; jiff just hit 1.0 |
| Tests | `insta`, `proptest`, `similar`, `scraper`, `cargo-deny` | Snapshots plus advisory and licence gates |
