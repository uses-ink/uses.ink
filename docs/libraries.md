# Libraries

Every dependency we take on purpose, and why. Versions are those checked on 2026-11-26; the
workspace `Cargo.toml` is the source of truth for the version in use, this page for the reason.
Evidence, alternatives and risks: `research/deps-engine.md`, `research/deps-frontend.md`,
`research/deps-infra.md`. Adding a dependency adds a row here in the same change.

## Engine, builder, CLI

| Need | Crate | Why |
|---|---|---|
| Markdown | `comrak` 1.2, no default features | wikilinks, embeds, callouts, block ids, spans (0027) |
| Frontmatter | `serde-saphyr` 2.2 | maintained, pure Rust, spans for evidence |
| `ink.toml` | `toml` 1.2 (`preserve_order`), `toml_edit` 0.25 | `toml` reads a spanned tree with recoverable errors, so every key and value has its span and an unknown key is found by walking it; `toml_edit` writes pins in place, keeping the author's formatting and comments |
| Git | `gix` 0.91: `blocking-network-client`, `parallel`, `pack-cache-lru-dynamic`, `revision`, `sha1`, in `host/native` only | pure Rust, shallow plus deepen, rev walk and tree diff for dates (0036); `blocking-network-client` is `git.fetch` (0046), and carries the file transport only until an HTTP transport is chosen |
| Hashing | `blake3` 1.8 | reference implementation |
| HTML safety | own allowlist; `ammonia` 4.2 behind an escape hatch only | smallest XSS surface (0027) |
| Highlighting capability | `arborium` 2.18, `arborium-theme` 2.18, in `host/native` only | tree-sitter grammars compiled from C, only the 14 languages enabled as features; `arborium-theme` maps capture names onto its slots, which become `model::highlight::Capture` (0039) |
| Math | `math-core` 0.9 | LaTeX to MathML Core, no JS for readers |
| Images | `image` 0.25 (GIF, JPEG, PNG and WebP decoders only), `fast_image_resize` 6.2, `ravif` 0.13 without `asm` and `threading`, `jpeg-encoder` 0.7 without `simd`, `imagesize` 0.15, thumbhash ported with `libm`; in `host/image` only | pure Rust decode, resize, AVIF and JPEG, no C or assembly; each pinned so the same bytes come out on every machine (0040); `imagesize` reads an AVIF's size, which nothing decodes without dav1d (C) |
| Fonts | `harfrust` 0.16, `skrifa` 0.50, `subsetter` 0.2, `ttf2woff2` 0.14 | maintained successors of rustybuzz and ttf-parser; pure Rust WOFF2 |
| Pen | own port of perfect-freehand, `kurbo` 0.13, `libm`, `tiny-skia` 0.12, `resvg` 0.50 | one deterministic implementation (0025); `freehand-web` is 36 KB gzipped, no `wasm-bindgen` (0034) |
| Feeds, sitemap | `atom_syndication` 0.12, `rss` 2.1, `quick-xml` 0.42 | typed feeds; no maintained sitemap crate |
| Text | `unicode-segmentation`, `unicode-normalization` | reading time, slugs |
| Plugins | `wasmtime` 51 (`component-model`, `cranelift`, `parallel-compilation`, `runtime`, `std`; no defaults), in `host/wasmtime` only | component model is Tier 1; fuel, epochs, a resource limiter and `.cwasm` (0035, 0041) |
| Plugin guests | `wit-bindgen` 0.63 (`macros`, `realloc`), in `plugin/sdk` and the `guest/` crates | guest bindings for `wasm32-unknown-unknown`, no WASI (0035) |
| Wrapping guests | `wit-component` 0.263, and `anyhow` 1 for its error type, in xtask only | turns a core module into a component, the version wasmtime 51 reads; no `wasm-tools` CLI or `cargo-component` |
| CLI | `clap` 4.6, `annotate-snippets` 0.12, `tracing`, `notify` 9, `notify-debouncer-full` 0.8, `indicatif` 0.18 | rustc's diagnostic renderer over our own `Diagnostic` |
| Preview server | `axum` 0.8 (`http1`, `tokio` only), `tokio` 1 (`rt`, `net`) | `ink preview` serves the manifest with its statuses and headers on a one-thread runtime; the same router stack `ink serve` will use, so no second HTTP crate |
| Dates | `jiff` 1.0 | chrono is unmaintained (0028) |
| Serialization | `serde` 1, `serde_json` 1 | typed JSON for `--json`, events and `cargo metadata`; the engine reads a vault's `.obsidian/app.json` with them (0043) |
| Tests | `insta`, `proptest`, `similar`, `scraper`, `cargo-deny` | snapshots, properties, licence and advisory gates |
| Benchmarks | `divan` 0.1 | `#[divan::bench]` beside the code with no harness to write, args and per-sample setup built in, a third of criterion's dependencies; benches live in `benches/`, are built by the gate's clippy and never run by it (docs/research/performance.md) |

| Repo tasks | `quote`, `proc-macro2`, `syn` 3, `prettyplease` 0.3, `heck`, `serde_json` | generated Rust is built as tokens and printed, never spliced; `cargo metadata` read as typed JSON; `syn`'s `visit` walks the module tree for the layout rules |

Later: `typst` and `typst-html` 0.15 for `ink:format-typst` (they still pull rustybuzz and
ttf-parser, so this waits); `pagefind` 1.6 for opt-in search.

## Control plane and edge

| Need | Crate | Why |
|---|---|---|
| Worker | `worker` 0.9 with `d1`, `http`, `axum`, `queue` | bindings for everything; container egress through `Reflect` (0037); 2.0 MB test build against a 64 MiB limit |
| Rows | own D1-shaped `Db` port (query plus atomic batch, raw SQL); `rusqlite` bundled for self-host | D1 has no interactive transactions and no sqlx path that matches current sqlx |
| SQL | `sea-query` builders over typed identifiers, rendered by one `db::Renderer`; `rusqlite` runs the text (0049) | no SQL text in code; the same statements for SQLite and D1 |
| Migrations | Rust values (`db::Migration`), own runner writing wrangler's `d1_migrations` ledger (0049) | one migration set, one ledger format |
| Self-host HTTP | `axum` 0.8, `tower-http` 0.7, `tokio`, `rustls` with `ring`, `instant-acme` | on-demand certificates only for names in the domain table |
| Outbound HTTP | own `Http` port: `worker::Fetch` hosted, `reqwest` native | one call site for forges, DoH, Cloudflare API |
| OAuth, GitHub App | `oauth2` 5, `jsonwebtoken` 12 (`rust_crypto`) | compile to wasm32 |
| Webhooks | `hmac`, `sha2`, `subtle` | GitHub, Forgejo HMAC; GitLab 19.1 Standard Webhooks signatures |
| Sessions | opaque cookie, hashed session rows | no signed state to rotate |
| Email | Cloudflare Email Service hosted, `lettre` self-host | magic links, receipts |
| DNS TXT | DoH JSON over the `Http` port, both targets | one implementation |
| Sealing | `chacha20poly1305` (XChaCha20-Poly1305), AAD = row id | `age` and `hpke` cannot share a build (pre-release `kem` conflict) |
| R2 uploads | `rusty-s3` for presigned PUTs | `ink push` uploads straight to R2 |
| Custom hostnames | plain REST through `Http` | the `cloudflare` crate has no endpoint for it and no release since 2025-03 |
| ATproto (later) | `atrium-identity`, `atrium-oauth` | carries `rsa` 0.9 advisories: reviewed exception or own DPoP client, decided when M-later starts |
| Ids | `uuid` v7 | sortable, standard |
| Release | `cargo-dist`; musl binary on `chainguard/static` (about 460 KB base) | one static binary, small image |
| Worker tests | vitest with miniflare | runs the built Worker against local D1 and R2 |

Passkeys (`webauthn-rs` 0.6 builds for wasm32) come after v1. `chrono` stays in the tree through
`worker` itself; we never use it directly.

## Dashboard and landing

| Need | Crate or tool | Why |
|---|---|---|
| Router | `axum` 0.8 over `worker` 0.9 hosted, tokio self-host | one router for both edges |
| HTML, SVG, CSS, behavior | `unspliced` (own crate, 0032) | typed names, baked documents, typed CSS, streamed pages, no JS; `no_std`, zero dependencies |
| Pen in the browser | `freehand-web`: plain wasm exports and a 40-line loader, `freehand.js` | one pen (0025), no generated JS (0034) |
| Themes | `wit-bindgen` 0.63 guest built for `wasm32-unknown-unknown`, wrapped with `wasm-tools component new` | a small theme stays small: `unspliced` has no dependencies |
| Slot overrides | `minijinja` in the host | 734 KB inside a theme, so it lives once in the host |
