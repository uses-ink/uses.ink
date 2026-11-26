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
| `ink.toml` | `toml` 1.2, `serde_spanned`, `serde_ignored`, `toml_edit` 0.25, `schemars` 1.2 | spans, unknown-key diagnostics, format-preserving pins, editor schema |
| Git | `gix` 0.91, minimal features | pure Rust, shallow plus deepen, rev walk and tree diff for dates |
| Hashing | `blake3` 1.8 | reference implementation |
| HTML safety | own allowlist; `ammonia` 4.2 behind an escape hatch only | smallest XSS surface (0027) |
| Highlighting | `giallo` 0.6 | VS Code grammars, classed spans, pure Rust; EUPL-1.2 is compatible with our AGPL |
| Math | `math-core` 0.9 | LaTeX to MathML Core, no JS for readers |
| Images | `image` 0.25, `fast_image_resize` 6.2, `ravif` 0.13, `jpeg-encoder` 0.7, `imagesize` 0.15, `img-parts` 0.4, vendored thumbhash | pure Rust AVIF and JPEG, no C |
| Fonts | `harfrust` 0.16, `skrifa` 0.50, `subsetter` 0.2, `ttf2woff2` 0.14 | maintained successors of rustybuzz and ttf-parser; pure Rust WOFF2 |
| Pen | own port of perfect-freehand, `kurbo` 0.13, `libm`, `tiny-skia` 0.12, `resvg` 0.50 | one deterministic implementation (0025); `freehand-web` is 36 KB gzipped, no `wasm-bindgen` (0034) |
| Feeds, sitemap | `atom_syndication` 0.12, `rss` 2.1, `quick-xml` 0.42 | typed feeds; no maintained sitemap crate |
| Text | `unicode-segmentation`, `unicode-normalization` | reading time, slugs |
| Plugins | `wasmtime` 51 (component model, fuel, epochs, pooling, `.cwasm`); `wit-bindgen` 0.63 for guests | component model is Tier 1 |
| CLI | `clap` 4.6, `annotate-snippets` 0.12, `tracing`, `notify` 9, `notify-debouncer-full` 0.8, `indicatif` 0.18 | rustc's diagnostic renderer over our own `Diagnostic` |
| Dates | `jiff` 1.0 | chrono is unmaintained (0028) |
| Serialization | `serde` 1, `serde_json` 1 | typed JSON for `--json`, events and `cargo metadata` |
| Tests | `insta`, `proptest`, `similar`, `scraper`, `cargo-deny` | snapshots, properties, licence and advisory gates |

| Repo tasks | `quote`, `proc-macro2`, `syn` 3, `prettyplease` 0.3, `heck`, `serde_json` | generated Rust is built as tokens and printed, never spliced; `cargo metadata` read as typed JSON |

Later: `typst` and `typst-html` 0.15 for `ink:format-typst` (they still pull rustybuzz and
ttf-parser, so this waits); `pagefind` 1.6 for opt-in search.

## Control plane and edge

| Need | Crate | Why |
|---|---|---|
| Worker | `worker` 0.9 with `d1`, `http`, `axum`, `queue` | bindings for everything; container egress through `Reflect` (0037); 2.0 MB test build against a 64 MiB limit |
| Rows | own D1-shaped `Db` port (query plus atomic batch, raw SQL); `rusqlite` bundled for self-host | D1 has no interactive transactions and no sqlx path that matches current sqlx |
| Migrations | wrangler's `.sql` folder; own runner for self-host writing the same `d1_migrations` table | one migration set, one ledger format |
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
