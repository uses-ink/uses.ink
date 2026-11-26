# uses.ink: frontend dependencies

Scope: what the dashboard (`app.uses.ink`, and `ink serve` self-hosted) and the reader
themes are built with, and how the pen becomes one shared component. Reads with
`architecture.md` sections 1 to 3.2 and `flow.md` section 8.

Method: versions and dates come from the crates.io API, the npm registry and PyPI, read on
2026-11-26. Claims that mattered were checked in clones under `~/code/strangers/`
(`workers-rs`, `leptos`, `maud`, `hypertext`, `datastar`, `datastar-rust`) or by building a
spike in a scratch directory. Spike numbers are from one laptop (Intel Core Ultra 5 135U,
14 threads), release builds, so read them as orders of magnitude. Anything not checked is
marked `[unverified]`.

Owner decisions this document builds on:

- The dashboard runs in a Worker (workers-rs 0.9 plus a Durable Object per handle) and in
  `ink serve` (axum), from the same code.
- Reader themes are WebAssembly components in v1: the builder hosts them with wasmtime, and
  a theme is a component that implements a WIT world. The default theme is quiet and calm;
  the doodle theme is opt-in.
- The pen becomes one Rust crate, used natively at build time and as wasm in the browser.

## Spikes run for this document

| Spike | What it shows | Result |
|---|---|---|
| `penspike`: port of `pen-core.js` (rng, hash, straight, humanise, ink, box) on the `perfect_freehand` 0.1.1 crate | Rust pen size in the browser, and parity with the JS pen | wasm after wasm-bindgen: 43,410 B raw, 19,220 B gzip, 16,844 B brotli. Native Rust output equals the JS output byte for byte along the stroke, then diverges at the end cap (the crate emits fewer cap points than the JS library) |
| `dashwasm`: workers-rs 0.9 `#[event(fetch)]` plus axum 0.8.9 router, maud 0.27 page, datastar 0.4.2 SSE handler | The hypermedia dashboard stack compiles for `wasm32-unknown-unknown` inside a Worker | Compiles first try, no tokio needed. 876,544 B raw, 222,872 B gzip before wasm-bindgen. Not deployed, so runtime behaviour is `[unverified]` |
| `themespike`: one WIT world (`render-page`, import `read-override`), guests in Rust (maud, minijinja), JS (jco componentize), Python (componentize-py), host on wasmtime 51 | Theme component size, instantiation and render cost, import surface | Table in section 7 |

Spike sources are in the session scratchpad, not in the repo.

---

## 1. Typed HTML in Rust

| Crate | Version (date) | Checks at compile time | Composition | Escaping | Speed | wasm | Activity |
|---|---|---|---|---|---|---|---|
| [maud](https://crates.io/crates/maud) | 0.27.0 (2025-02-02) | Syntax only; element and attribute names are not checked | Plain Rust functions returning `Markup`; `Render` trait | Automatic; `PreEscaped` to opt out | Fast, writes into one `String` | Pure macro, no runtime deps; ran in both spikes | 2,650 stars, `main` pushed 2026-11-14; unreleased work (Rust 2024, HTML-safe JSON embed) |
| [hypertext](https://crates.io/crates/hypertext) | 0.12.1 (2025-08-10) | Elements and attributes checked against marker types; `data-*` and quoted names skip checks; htmx and Alpine attribute sets behind features | `#[component]` and `#[renderable]`, lazy rendering | Automatic; raw output only through `dangerously_*` APIs since 0.12 | Claims top of the [template benchmark](https://github.com/askama-rs/template-benchmark#benchmark-results) | `#![no_std]` | 290 stars, single maintainer. SVG validation (`svg::maud!`, automatic context switch inside `<svg>`) merged 2026-03-22 (`feat: svg (#187)`) but **not released**: docs.rs 0.12.1 has no SVG module |
| [askama](https://crates.io/crates/askama) | 0.16.1 (2026-09-04) | Template syntax and field access checked at compile time against a Rust struct | Jinja inheritance, includes, macros | Automatic, per extension | Fast | Compiles to wasm `[unverified, no spike]` | 1,263 stars, active. [rinja](https://crates.io/crates/rinja) is `0.4.0+deprecated`: merged back |
| [minijinja](https://crates.io/crates/minijinja) | 3.1.0 (2026-11-07) | None: templates are data, errors at render time, with good messages | Jinja inheritance, blocks, macros | Automatic by file extension | Slower than compiled templates (section 7: 68 µs per page against 18 µs for maud, inside a component) | Ran in a component | Very active (17M recent downloads) |
| [horrorshow](https://crates.io/crates/horrorshow) | 0.8.9 (2026-02-15) | Syntax only | Closures | Automatic | Fast | Pure macro | Low activity |
| [markup](https://crates.io/crates/markup) | 0.16.0 (2026-01-24) | Syntax only | Template structs | Automatic | Fast | Pure macro | Small |
| [sailfish](https://crates.io/crates/sailfish) | 0.11.9 (2026-10-26) | EJS-style files, compiled | Includes | Automatic | Very fast | `[unverified]` | Active |
| [typed-html](https://crates.io/crates/typed-html) | 0.2.2 (2020-02-07) | Full DOM typing | JSX-like | Automatic | Slow compiles | | Dead; hypertext is its practical successor |

Is maud okay? **Yes.** It is the safe default: small, stable, widely used, writes HTML that
looks like the mocks, escapes by default, and compiles anywhere (both spikes used it). What
it lacks is name checking: `clas="x"` compiles. hypertext fixes exactly that with nearly the
same syntax, but its SVG support is unreleased and the pen emits SVG everywhere, so today
that means a git dependency on a one-person crate.

**Recommendation: maud** for the dashboard and for first-party themes. Revisit hypertext when
a release ships the SVG work; the move is mostly mechanical because `hypertext::maud!`
follows maud syntax `[unverified how many call sites would need edits]`. Use **minijinja**
only where templates must be data at runtime: author overrides (section 7). Askama adds a
second template language with no gain over maud for code we own.

## 2. Full component frameworks

| Framework | Version (date) | SSR in a Worker | Same crate in axum | Islands or partial hydration | Zero-JS static output | Notes |
|---|---|---|---|---|---|---|
| [Leptos](https://crates.io/crates/leptos) | 0.8.23 (2026-10-31); **0.9.0-rc1** of `leptos_axum` and `leptos_router` the same day | **Yes, official.** `workers-rs/templates/leptos`: `leptos_axum` with `default-features = false, features = ["wasm"]`, `worker` 0.9, server functions, assets via Workers Assets | Yes: `leptos_axum` default features run on tokio (`init_executor` in `integrations/axum/src/lib.rs` picks wasm-bindgen or tokio by feature) | `islands` feature. The [Leptos book](https://book.leptos.dev/islands.html) cites 24 KB of wasm for an islands example against 274 KB without islands, uncompressed | Yes: render a view with `to_html()` and ship no hydration script; `SsrMode::Static` and `generate_static_files` in `router/src/generate_route_list.rs` | Two builds (`ssr` and `hydrate`), `cargo-leptos`, 0.9 is about to land, so expect API churn |
| [Dioxus](https://crates.io/crates/dioxus) | 0.7.10 stable (2026-07-30); 0.8.0-alpha.3 (2026-10-16) | **No official path.** The third-party `dioxus-cloudflare` crate patched `dioxus-server` with cfg gates because 0.7.3 did not compile for `wasm32-unknown-unknown`; today crates.io lists it as `0.0.0` (2026-02-24, 42 recent downloads) and its GitHub repo returns 404 | Yes, `dioxus-fullstack` on axum | No islands | `dioxus-ssr` renders strings | Strongest for desktop and mobile, which uses.ink does not need |
| [Sycamore](https://crates.io/crates/sycamore) | 0.9.4 (2026-11-09); 0.10.0-beta.1 | `[unverified]` | `[unverified]` | No | SSR to string | 22k recent downloads; small |

Cold start: the dashboard spike (maud plus datastar plus axum) is 223 KB gzip before
wasm-bindgen; a Leptos SSR server build carries the reactive runtime and router as well, so
expect a larger Worker `[unverified, not built]`. Workers start isolates in parallel with
the TLS handshake and the limit is 1 s of startup CPU (architecture section 3.2 cites the
limits page), so neither is likely to be close, but only a deploy measures it.

Can Leptos render the static reader with zero client JS? Technically yes. It no longer fits
the plan: themes are components now (section 7), and a reactive framework inside a theme
component buys nothing for HTML that never hydrates. For the dashboard, Leptos is the only
full framework with a working Worker story, at the price of a second build, a wasm client
bundle, `cargo-leptos`, and a 0.9 migration soon.

**Recommendation: no full framework.** The dashboard is four screens of server state plus
one live log. Leptos is the fallback if the dashboard later grows real client state (drag
to reorder, an editor); its Worker template proves the path.

## 3. Hypermedia: maud plus htmx, Datastar or alpine-ajax

| Library | Version (date) | Size | SSE | CSP | Rust side |
|---|---|---|---|---|---|
| [Datastar](https://github.com/starfederation/datastar) | v1.0.5 tag (repo pushed 2026-11-19). The npm package `@starfederation/datastar` is stale at 1.0.0-beta.11; ship the bundle from the repo or CDN | `bundles/datastar.js` 14,140 B gzip; `datastar-core.js` 5,311 B gzip (measured) | Native: every response can be an SSE stream of `patch-elements` and `patch-signals` events | Default compiles expressions with `Function(...)`, which needs `'unsafe-eval'`. With a nonce on `<html>`, it compiles through nonce-tagged script elements and a Trusted Types policy (`library/src/engine/csp.ts`) | [datastar](https://crates.io/crates/datastar) 0.4.2 (2026-11-21): axum, actix, rocket, warp; passes the official SDK test suite; axum feature pulls no tokio (checked `Cargo.toml`, and the `dashwasm` spike compiled for the Worker) |
| [htmx](https://www.npmjs.com/package/htmx.org) | 2.0.11 latest (2026-09-22); 4.0.2 on `next` | `[unverified, not measured]` | SSE via an extension | No eval for core attributes `[unverified]` | [axum-htmx](https://crates.io/crates/axum-htmx) 0.8.1 (2025-06-05): request and response headers. hypertext has htmx attribute checking |
| [alpine-ajax](https://www.npmjs.com/package/@imacrayon/alpine-ajax) | 0.12.7 (2026-03-17) on Alpine 3.17.5 | `[unverified]` | No | Alpine needs its CSP build to avoid eval `[unverified]` | None |

The live build log is the deciding feature. With Datastar the Deploys screen opens one SSE
connection; the Worker relays the Durable Object's event stream, and each event is a maud
fragment appended to `#log`. Self-hosted, the same handler reads a `tokio::sync::broadcast`
(the `axum-watch` example does this). The handler is the same function in both hosts. Only
the event source behind a port differs, which matches the ports in architecture section 1.

Two things to handle:

- Do not use axum's `Sse` keep-alive on the Worker: it is behind axum's `tokio` feature
  (`response/sse.rs`). Send a comment line on a timer from the event source instead, or rely on the
  Durable Object to emit progress often enough.
- The dashboard is rendered per request, so a per-response nonce is cheap. Set the nonce on
  `<html>` and Datastar runs without `'unsafe-eval'`.

**Recommendation: maud plus Datastar**, nonce mode, the bundle vendored and served by the
dashboard itself (no CDN).

## 4. TypeScript option, for comparison

| Piece | Version (date) | On Workers | In `ink serve` |
|---|---|---|---|
| [SvelteKit](https://www.npmjs.com/package/@sveltejs/kit) | 3.4.1 (2026-11-25), adapter-cloudflare 9.0.1 | Yes | Needs Node or Bun next to the Rust binary, or a static SPA build served by axum |
| [SolidStart](https://www.npmjs.com/package/@solidjs/start) | 2.2.3 (2026-11-16) | Yes | Same |
| [Astro](https://www.npmjs.com/package/astro) | 7.4.13 (2026-11-18), @astrojs/cloudflare 14.4.9 | Yes; the Astro team joined Cloudflare on 2026-01-16 ([press release](https://www.cloudflare.com/press/press-releases/2026/cloudflare-acquires-astro-to-accelerate-the-future-of-high-performance-web-development/)) | Same |
| Type bridge | [ts-rs](https://crates.io/crates/ts-rs) 12.0.1 (2026-01-31); [specta](https://crates.io/crates/specta) 2.0.0-rc.28 (stable line 1.0.5); [typeshare](https://crates.io/crates/typeshare) 1.0.5 (2026-01-02) | | |

The only TS shape that keeps "same code for both hosts" is a static SPA (SvelteKit
`adapter-static` or Solid) that talks to the JSON API, served from Workers Assets hosted and
embedded in the `ink` binary self-hosted. Costs: a second language and toolchain (npm,
bundler, lockfile, audits), generated types to keep in sync, the pen either duplicated in TS
or loaded as wasm anyway, and a client bundle where the hypermedia option ships 14 KB of
Datastar. Gain: a mature component model and a large ecosystem.

**Recommendation: no.** The owner would carry two languages to avoid HTML and CSS, which
the TS frameworks still require.

## 5. CSS without the vanilla fear

| Tool | Version (date) | What it does | Fit |
|---|---|---|---|
| [lightningcss](https://crates.io/crates/lightningcss) | 1.0.0-alpha.72 (2026-07-20); still alpha after years, used by cargo-leptos and turf | Parse, lower nesting and custom media for a browser list, bundle `@import`, minify, Rust API | **Use it** in the build: bundle and minify the dashboard CSS and each theme's CSS, and parse author `custom.css` so a syntax error is a `Diagnostic` with a line number instead of a broken page |
| [stylance](https://crates.io/crates/stylance) | 0.8.4 (2026-06-15) | CSS modules: `import_style!` turns class names into Rust constants (unknown class is a compile error, unused is a warning); a CLI bundles the hashed CSS | Good for the dashboard: one `.module.css` next to each maud component. Framework-agnostic |
| [turf](https://crates.io/crates/turf) | 0.10.1 (2025-05-02) | SCSS through grass at compile time, hashed classes, lightningcss minify, CSS embedded in the binary | Works, but SCSS is a third language and the repo has been quiet since May 2025 |
| [stylist](https://crates.io/crates/stylist) | 0.15.1 (2026-03-18) | CSS-in-Rust, runtime injection, Yew-centred | Runtime styles conflict with a strict CSP and a static reader. No |

Design tokens, one source:

- A `uses-tokens` crate holds `tokens.toml` (colours `ink`, `pen`, `bucket`, `sky`, `mint`,
  `pink`, `cloud`; pen weights; the `SHIFT` offset; font stacks; measure).
- Its `build.rs` writes Rust constants (the pen reads colours and weights from them) and a
  `tokens.css` with `:root { --ink: #000; ... }`. The dashboard CSS and the first-party theme
  CSS use only `var(--...)`.
- Themes receive the same tokens as a typed WIT record (section 7), so a component theme
  never hardcodes a colour the author can override.

This is one small build script; no token tool such as Style Dictionary is needed.

**Recommendation:** plain CSS files with nesting, tokens generated from `tokens.toml`,
lightningcss for bundling, minifying and validating, stylance for dashboard class scoping.
Owner fear of vanilla CSS is mostly fear of global names and drift; stylance removes the
first and generated tokens remove the second.

## 6. The pen in the browser

| Option | Size on the wire | One implementation | Notes |
|---|---|---|---|
| Current JS (`pen.js`, `pen-core.js`, vendored perfect-freehand) | 7,272 B gzip for all three, unminified (measured) | No: a Rust pen is needed at build time anyway | Already works |
| Rust pen crate as wasm for the geometry, small JS for the DOM (measure boxes, observers, frames) | 19,220 B gzip (16,844 B brotli) for the core in the spike, plus about 1.6 KB of wasm-bindgen glue and about 3.6 KB of hand JS | Yes | Needs `script-src 'wasm-unsafe-eval'` in the dashboard CSP |
| Rust plus web-sys doing the DOM work too | Larger `[unverified]` | Yes | More binding code for no gain |
| Custom element (`<ink-box>`, `<ink-line>`) wrapping the wasm | Same as option 2 | Yes | Gives the mocks' `data-box` and `data-fill` a real tag with attributes, and keeps the JS glue in one file |

Spike findings:

- The port matched the JS pen byte for byte along a whole stroke (same mulberry32 and FNV
  hash, same humanise maths, `Math.sin` in V8 and Rust `f64::sin` agreed on this sample).
- The [perfect_freehand](https://crates.io/crates/perfect_freehand) crate (0.1.1,
  2026-01-27, 4 stars, zero deps, MIT) differs from the JS library at the end cap. Vendor it
  into the pen crate and fix it against the JS output, or port the 400-odd lines yourself.
  Either way, keep a golden test that compares SVG output with the JS reference for a set of
  seeds until the JS pen is deleted.
- Formatting with `{:.2}` cost 25 KB of wasm (68 KB down to 43 KB raw when replaced by
  integer formatting). Keep float formatting out of the pen.
- Native and wasm builds call different `sin` implementations (platform libm natively,
  Rust's libm on wasm) `[unverified that they can differ in the last bit]`. A last-bit
  difference is enough to flip a two-decimal rounding. Use the `libm` crate in the pen so
  the builder and the browser draw identical marks.

**Recommendation:** one `uses-pen` crate (`no_std` plus `alloc`, pure functions from geometry
and seed to SVG path data). The builder links it natively. The dashboard gets it as wasm
behind one custom element file. The doodle theme links it inside its component
(section 7). Delete `pen-core.js` once the golden test passes.

## 7. Reader themes as WebAssembly components

### 7.1 Authoring a theme in Rust

- **No `cargo-component` needed.** It was last released at 0.21.1 on 2025-03-18 and its
  repo was last pushed 2025-07-14; its README says plain `cargo` with the upstream
  `wasm32-wasip2` target is "not a complete replacement" for custom WIT. The spike shows
  that is outdated for our case: [wit-bindgen](https://crates.io/crates/wit-bindgen) 0.63.0
  (2026-10-14) `generate!` with a custom world builds with plain `cargo build`.
- Two build routes, measured with the same guest code:

| Route | Imports in the finished component | maud guest | minijinja guest |
|---|---|---|---|
| `--target wasm32-wasip2` | maud: `wasi:cli/*`, `wasi:io/*`, `wasi:clocks/monotonic-clock`. minijinja adds `wasi:clocks/wall-clock` and `wasi:random/insecure-seed` (from `HashMap` seeding) | 50,874 B | 807,522 B |
| `--target wasm32-unknown-unknown` then `wasm-tools component new` | **Only the theme world** (`read-override`) | 19,372 B (8,028 gzip) | 734,404 B (274,658 gzip); 610,998 B with `opt-level = "z"` |

  The second route is the one to standardise: a component that imports nothing but the
  theme world cannot read a clock, a random seed or the environment, so it is deterministic
  by construction, and the host can refuse any theme whose import list goes beyond the world.
- maud runs fine in a guest. hypertext is `no_std` and should too `[unverified, not built]`.
  askama should `[unverified]`. minijinja runs and works with `default-features = false`.

### 7.2 Host cost with wasmtime

[wasmtime](https://crates.io/crates/wasmtime) 51.0.0 (2026-11-20), `bindgen!` host,
`InstancePre` created once, one fresh `Store` and instance per page (2,000 pages, a 5 KB
page body), fuel and epoch interruption on:

| Theme | Precompile | `.cwasm` | Deserialize | Instantiate (pooling / on demand) | Instantiate plus render | Fuel per page |
|---|---|---|---|---|---|---|
| Rust, maud | 11.5 ms | 86,536 B | 0.16 ms | 5.5 µs / 8.3 µs | **18 µs** | 10,724 |
| Rust, minijinja with an override | 349 ms | 2,977,256 B | 1.19 ms | 5.4 µs / 8.2 µs | **68 µs** | 252,849 |
| JS, jco 1.42.0 `componentize --disable all` (componentize-js 0.24.0, StarlingMonkey) | 3.65 s | 43,129,744 B | 19 ms | 5.2 µs (pooling) | **1.58 ms** | 14,760,712 |
| Python, componentize-py 0.25.1 | not run | | | | | |

Reading the numbers:

- A fresh instance per page is cheap enough to be the rule (5 to 8 µs). Per-page isolation
  means no state leaks between pages, so rebuilding one page gives the same bytes as a full
  build. The pooling allocator saves about 3 µs per instance; worth turning on, not decisive.
- Precompile once per theme version and cache the `.cwasm` keyed by component hash,
  wasmtime version and engine config. `Component::deserialize` is `unsafe`, so only load
  `.cwasm` files the builder wrote itself.
- Fuel is deterministic and per page; set the budget from the first-party theme times a
  large factor, and add an epoch deadline for wall time. JS themes burn about 1,400 times the
  fuel of a maud theme for the same page, so the budget must be per language or generous.
- A 1,000-page site: under 0.1 s for a maud theme, about 1.6 s for a JS theme, in render
  alone, single-threaded.

Other languages:

- **JS/TS via jco:** 11,976,020 B component (3.8 MB gzip), works, and with `--disable all`
  imports no WASI at all (verified). Caveat seen in the spike: importing a world-level
  function (`read-override`) from JS failed with "module '<internal>' doesn't provide an
  export"; put host imports in a named WIT interface (`import host;`) so JS can import them by
  name `[unverified that this fixes it]`.
- **Python via componentize-py:** 18,955,794 B (6.4 MB gzip) for a ten-line theme, and the
  component imports 25 WASI interfaces (CPython needs them). The wasmtime host refused it
  without a WASI implementation (`instance export pollable has the wrong type`). Supporting
  it means linking `wasmtime-wasi` with a fixed clock, a seeded random source and an empty
  filesystem `[unverified that every import can be stubbed deterministically]`. Leave Python
  out of v1.

### 7.3 How authors override a component theme

Survey of other tools:

| Tool | Tokens or settings | Custom CSS | Template overrides | Full theme |
|---|---|---|---|---|
| Zola (tera) | `[extra]` in `config.toml` | Site `static/` | Same path in site `templates/` wins; can override one block ([docs](https://www.getzola.org/documentation/themes/extending-a-theme/)) | `theme = "name"` from `themes/` |
| Hugo | `params` | Assets pipeline | Lookup order: project `layouts/` before theme; theme components stack | Modules |
| mdBook | CSS variables (`--content-max-width`, `--sidebar-width`) | `additional-css` in `book.toml` | Same-named file in `theme/` (`index.hbs`, `head.hbs`, ...) replaces the default ([docs](https://rust-lang.github.io/mdBook/format/theme/index.html)) | Copy with `mdbook init --theme` |
| Quartz | `quartz.config.ts` | `custom.scss` | Layout of TSX components in `quartz.layout.ts` ([docs](https://quartz.jzhao.xyz/layout)) | Fork |
| Obsidian Publish | Theme from the community list | `publish.css` in the vault root | None | `publish.js` only on a custom domain ([docs](https://obsidian.md/help/publish/customize)) |
| Bear Blog | Built-in themes | One "Edit theme CSS" field, `@import` allowed; per-post `class_name` header ([docs](https://docs.bearblog.dev/styling/)) | None | None |
| Ghost | `config.custom` in the theme's `package.json`: select, boolean, color, image, text, read as `@custom` ([docs](https://docs.ghost.org/themes/custom-settings)) | Code injection | Edit the Handlebars theme | Zip upload |
| WriteFreely | None | Per-blog custom CSS (Pro on Write.as); instance-wide `static/local/custom.css` ([docs](https://writefreely.org/docs/main/admin/design)) | None | None |

Pattern: every tool that lasts offers the same three rungs, in order of how often people
use them: settings, extra CSS, then replacing one template piece. Full themes are rare.

Proposed rungs for uses.ink, all in `ink.toml` plus a `theme/` folder, none required:

1. **Tokens.** The theme ships a manifest listing its tokens with types and defaults
   (colour, length, font stack, boolean). `[theme.tokens]` in `ink.toml` sets them. The host
   checks types, then passes them to the component as a typed record, and the theme writes
   them as CSS custom properties. Unknown token is a `Diagnostic`, as with every other key.
2. **Custom CSS.** `theme/custom.css` is appended by the host after the theme CSS, through
   lightningcss. No theme cooperation needed, works for every theme in every language.
3. **Slot overrides.** The theme declares named slots (`head-extra`, `header`, `footer`,
   `post-meta`, `after-post`) in its WIT world as a host import:
   `slot: func(name: string, ctx: slot-context) -> option<string>`. The host looks for
   `theme/<slot>.html`, renders it with its own **native minijinja** against a typed
   context, and returns the HTML. When no file exists, the theme draws its default.
4. **Fork.** `theme = "./my-theme.wasm"` or a path to a theme crate, for people who want it
   all.

Why slots on the host side rather than minijinja inside each theme: the spike shows
minijinja inside a component costs 600 to 730 KB and 50 µs per page per theme, and each
theme author would have to wire it up. With host-side slots every theme, in any language,
gets the same override syntax for free, the component stays at 19 KB, and the host owns the
escaping rules for author files. The `read-override` import in the spike (theme embeds
minijinja and pulls the file itself) works and stays possible for a theme that wants deeper
template control, but it should not be the contract.

This also answers "least hacky in a zero-config product": nothing is configured until the
author creates a file or a key, each rung is visible in "What we understood" as a
`Decision` (`theme.slot.footer = theme/footer.html`), and none of them reaches into the
theme's internals.

**Recommendation:** WIT world `uses:theme@0.1` with exports `render-page`, `render-index`,
`stylesheet`, and imports limited to `slot`, `asset` (read a theme or site asset by hash),
`measure-text` (section 8) and `pen` (section 8). Guests built with
`wasm32-unknown-unknown` plus `wasm-tools component new`; the host rejects any extra import.
First-party themes in Rust with maud. JS themes allowed later through jco with
`--disable all`. Python not in v1.

## 8. Hand-drawn marks in a no-JS reader page

The current mock draws marks once and stretches them with `background-size: 100% ...` and
`border-image ... stretch`. Stretching distorts the stroke: a 300 px underline stretched to
700 px gets thick ends and long flat wobble.

| Technique | No JS | Any element size | Wrapping text | Colour from tokens | Cost | Verdict |
|---|---|---|---|---|---|---|
| Build-time inline SVG per element, size from measured text | Yes | Only when width is known at build time | No | Yes | Inline bytes per mark | Use for single-line elements |
| Same, sized in `em` | Yes | Scales with font size without distortion | No | Yes | Same | Use: measured advance in font units maps to `em` exactly |
| `border-image` with `round` or `space` and a tileable edge | Yes | Yes | Block elements only | Through a mask, or one file per colour | One small SVG | Use for boxes (`pre`, figures, callouts) |
| `mask-image` with a repeating stroke tile, `background: var(--pen)` | Yes | Yes | With `box-decoration-break: clone` `[unverified for mask on wrapped inline boxes]` | Yes | Small | Use for link underlines and rules |
| Stretched SVG with `vector-effect: non-scaling-stroke` | Yes | Yes | No | Yes | Small | No: Firefox ignores it in CSS backgrounds ([bug 1530525](https://bugzilla.mozilla.org/show_bug.cgi?id=1530525)), and pen marks are filled outlines, not strokes |
| SVG filter (`feTurbulence` plus `feDisplacementMap`) on plain CSS borders | Yes | Yes | Yes | Yes | Filter cost on large areas | Fallback only: wobble looks like noise, not the pen's overshoot and bow |
| SVG `<symbol>` reuse | Yes | Fixed sizes | No | Yes | Small | Use for icons, bullets, circled numbers |
| CSS Houdini Paint API | No: needs `CSS.paintWorklet.addModule` from JS, Chromium only | | | | | No |

What real sites do: the hand-drawn look on the web is usually raster or pre-drawn SVG
images (xkcd), or JS at load time (rough-notation, Excalidraw) `[unverified survey, no
production no-JS example of per-element generated marks found]`. The approach below is
uncommon because it needs a build step that knows the text, which uses.ink has.

How the doodle theme draws without stretching:

1. **Measure at build time.** The host offers `measure-text(font, size, text) -> advance`
   to themes, backed by a native shaper ([harfrust](https://crates.io/crates/harfrust)
   0.16.1, 2026-11-19, the Rust HarfBuzz port; [skrifa](https://crates.io/crates/skrifa)
   0.50.0 for font tables). One shaper in the host, none duplicated in each component. Only
   fonts that the theme ships are measured, so the result matches the browser as long as the
   font loads; `font-display: block` on the hand font keeps it from swapping late.
2. **Draw per element.** For the site name, nav items, `h2` underlines, tags and circled
   sidenote numbers, the theme calls the pen with the measured width and a seed from the
   element's text, and inlines the SVG sized in `em`. Each mark is unique and undistorted.
3. **Tile what wraps.** For multi-line links and paragraphs-wide rules, the pen draws a
   periodic segment whose ends meet at the same height and slope, used as `mask-image` with
   `repeat-x` or as `border-image` with `round`. Several seeds give several tiles; the
   theme picks one per element by seed so neighbours differ.
4. **Boxes.** `border-image` nine-slice with pen-drawn corners and tileable edges,
   `border-image-repeat: round`.
5. **Optional motion.** Stop-motion boil without JS: two or three pre-drawn variants swapped
   by CSS `@keyframes` with `steps()`, off under `prefers-reduced-motion`. The draw-in
   animation of the dashboard needs the live pen and stays dashboard-only.

Inline SVG and same-origin SVG files pass a strict CSP (`default-src 'self'`; `img-src
'self'`); avoid `data:` URIs in CSS so `img-src` does not need `data:`.

**Recommendation:** the doodle theme uses build-time marks from the shared pen, sized from
host-measured text for single-line elements, tiled masks for wrapping text, nine-slice
borders for boxes. The quiet default theme needs none of this.

---

## Recommended dashboard stack

- **axum 0.8 router** as the single app, served by workers-rs 0.9 (`http` and `axum`
  features, as in `workers-rs/examples/axum`) hosted and by `ink serve` self-hosted.
- **maud 0.27** components, one Rust function per piece (tab, box, deploy row, decision row).
- **Datastar 1.0.5** bundle vendored, **datastar 0.4.2** crate for SSE fragments, nonce CSP.
  Live build log: Durable Object event stream hosted, `broadcast` channel self-hosted, same
  handler behind a port.
- **CSS:** plain nested CSS, stylance for scoping, tokens generated from `tokens.toml`,
  lightningcss to bundle and minify.
- **Pen:** `uses-pen` crate as wasm behind one custom element; about 20 KB gzip.
- Verified: the whole server side compiles for the Worker (`dashwasm` spike, 223 KB gzip
  before wasm-bindgen).

Main risk: **workers-rs ergonomics, not the UI stack.** The router, maud and datastar
compile, but streaming a long SSE response from a Durable Object through axum on the Worker,
with `?Send` futures and no tokio timers, has not been run. Spike it first: one deploy
page that streams a fake build log from a DO, deployed, open for ten minutes.

## Recommended theme architecture

- A theme is a **WebAssembly component** implementing `uses:theme@0.1`, built for
  `wasm32-unknown-unknown` and wrapped with `wasm-tools component new`, importing nothing
  outside the theme world. The host rejects extra imports, so themes are deterministic and
  cannot reach the network or a clock.
- Host: **wasmtime 51**, `InstancePre` per theme, a fresh instance per page with the pooling
  allocator, cached `.cwasm`, fuel per page plus an epoch deadline.
- First-party themes in **Rust with maud** (19 KB component, 18 µs per page). The quiet
  default first; doodle opt-in, linking `uses-pen` and calling host `measure-text`.
- Author overrides in four rungs: tokens in `ink.toml`, `theme/custom.css`, slot files
  rendered by host-side minijinja, then a full fork. Each rung shows as a `Decision`.
- Third-party themes later in Rust or JS (jco, 12 MB, 1.6 ms per page); Python not in v1.

Main risk: **the WIT world becomes a public API on day one.** Every slot name, context field
and token type that third-party themes rely on is frozen at `0.1`. Keep the first world
small (page, index, stylesheet, four or five slots), version it, and ship two first-party
themes on it before opening it, so the second theme tests the contract the first one shaped.
A smaller risk: text measured at build time drifts from the browser when the font fails to
load, so the doodle theme must still look right with a stretched mark as the fallback.
