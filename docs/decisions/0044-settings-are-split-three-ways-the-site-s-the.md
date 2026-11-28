---
number: "0044"
title: "Settings are split three ways: the site's, the operator's, and the fixed"
status: accepted
date: 2026-11-26
area: config
supersedes: []
---

## 0044 Settings are split three ways: the site's, the operator's, and the fixed

Accepted, owner. The owner asked for the hardcoded values and limits to be configurable. The
inventory is `research/configuration.md` (97 rows). Every value is one of three kinds, and the
kind decides where it can be set:

1. **Site-owner settings**, in `ink.toml`: the image width ladder and the AVIF and JPEG quality,
   the picture `sizes` hint as a theme token, the theme's other tokens, and perhaps the length a
   description is cut at.
2. **Operator limits**, in CLI flags or the host's own configuration: the cache bound, the image
   pixel budget, workers and images opened at once, the plugin budget (fuel, time, memory, table,
   epoch tick), the git read's threads, delta cache, link hops and block, the folder read's four
   budgets, and AVIF speed. `ink.toml` may only lower an operator limit, never raise it: a hosted
   source is untrusted, and what a builder spends on it is the operator's to say. The operator's
   value is the ceiling and the site's value is `min(site, ceiling)`; a key above the ceiling is a
   diagnostic that names both numbers, not a silent clamp. The markdown format's `DEPTH` and tag
   `NAME` length are bucket 2 too and live in `crates/runtime/plugin/format`, which this change did not touch.
3. **Never configurable**: schema and wire strings, `PREFIX`, `MARK`, `ENTRY`, `START` and
   `VERSION` constants, `Hash::DIGITS`, `SLUG_DIGITS` and the route hash digits (they change URLs
   and hashes), XML namespaces, `NONBLOCK`, `MIB` and `UNITS`, the MathML allowlist, the `OPAQUE`
   tags, the CSP and the other security headers, and the immutable `Cache-Control` value. A
   weaker value would be a hole a source could ask for, or a new identity for everything built.

The heuristic lists (content, post and template folder names, home and changelog stems,
`WRITTEN = "en"`) are inference rules. They belong to 0042's versioned rules (M4) and get no
settings of their own.

Three rules bind every setting that is added:

- **An output-affecting setting reaches the cache key, the deploy hash and the change set.** In
  the image work that is `host_image::Key`, which hashes every field of `Settings` and the pixel
  limit; a test destructures `Settings` whole, so a new field cannot be added without being
  hashed. A setting that changes output also changes the deploy hash because the output bytes
  change. A change in a setting is reported in 0042's change set as cause `config`, never
  `content`. A limit that changes no byte (workers, threads, tick) stays out of the keys.
- **New keys go through `config`'s `Value` and `Shape`.** A key is a type implementing `Value`,
  so the editor schema (architecture section 10) stays generated from one place and an
  out-of-range value is a typed error at the config boundary. The silent clamps in
  `crates/runtime/host/image/src/encode.rs` stay as the last guard and are not a substitute.
- **A limit carries its number.** Where a limit is checked, the error holds the limit and the
  value asked for, as typed fields: `Reason::Budget { width, height, limit }`,
  `Failure::Memory { limit, wanted }`, `Failure::Table { limit, wanted }`,
  `Refused::Looped { limit, .. }`, `Error::Tick { tick }`.

**Built (2026-11-27), host side only, behavior with the defaults byte-identical:**
`host_image::Limits` (`pixels`, `workers`, `opening`), carried by `Pixels::with_limits`, with
`Settings` left holding only what changes bytes (`widths`, qualities, speed);
`host_wasmtime::Budget` gains `table` and `tick`; `host_native::GitLimits` (`threads`, `deltas`,
`hops`, `block`) is `Repository.limits`; `Disk::with_bound`. Every default equals the old value,
and every existing golden is unchanged.

**Next, after 0042's step 1 lands** (the `crates/core/config/` and `crates/surface/cli/` code is another change's):

- `config`: an `[image]` table (`widths`, `avif-quality`, `jpeg-quality`) as `Value` types, read
  into `host_image::Settings`; a `sizes` token in `[theme]` for the quiet theme; each key
  clamped to the operator's ceiling with a diagnostic naming both numbers.
- `cli`: `Caching::pixels` builds `Pixels::cached(Settings::default(), ..)` and `Caching::new`
  builds the `Disk`; the flags `--cache-bound`, `--image-pixels`, `--workers` and `--image-speed`
  plug in there. `site.rs` builds `Repository { revision, output, .. }`, where `--git-threads`
  and its siblings plug in as `Repository.limits`. `component.rs` builds the `Runtime`, taking
  `Budget` for `--plugin-time`, `--plugin-memory` and `--plugin-fuel`. A hosted builder takes
  the same structs from its own configuration.
- `engine`: a changed setting becomes a change with cause `config` in 0042's change set; the
  description lengths wait for the owner to choose between a setting and a rule.
- `crates/runtime/plugin/format/markdown`: `DEPTH` and the tag `NAME` length become `Budget` fields passed
  through the format call; the entity lookahead (an inline `12`) is named first.

**Why:** the values were scattered consts and literals with no one place to raise or lower
them; a split by who may set a value keeps hostile sources from spending a builder's resources
and keeps identity values out of anyone's hands. **Reversal:** a bucket 3 value turns out to
need per-site variation (the placeholder size, for instance): it moves to bucket 1 with its
field added to the cache key first.
