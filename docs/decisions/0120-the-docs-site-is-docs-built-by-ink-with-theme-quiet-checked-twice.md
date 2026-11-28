---
number: "0120"
title: "The docs site is `docs/` built by `ink` with theme-quiet, and the sites stage builds it twice and refuses any diagnostic"
status: built
date: 2026-11-28
area: release
supersedes: []
---

## 0120 The docs site is `docs/` built by `ink` with theme-quiet, and the sites stage builds it twice and refuses any diagnostic

Built (2026-11-28), the dogfood item of M5. `docs/site.md` says how to build and read it.

- **The site is the folder.** `docs/ink.toml` is its configuration: an `exclude` list, `ink:theme-quiet`,
  a hand-written home (`docs/index.md`) and a pinned `site.nav`. The mechanism is the one every writer has
  (0018, 0042): no page gets a `publish` key, since the list is the one place that says what is public and
  `ink select` replays it. The README and `PRODUCT.md` are outside the folder and not published: the first
  is for contributors, the second is the design brief.
- **Excluded, not allowed.** A folder source publishes what it holds, so the list names what is not for a
  reader: `research/`, the pages for people who work on the code, the owner's pages. `history/review-fixes.md`
  and `crates.md` stay because the decisions index and the architecture link to them.
- **A diagnostic fails the stage.** `cargo xtask check sites` builds `docs/` with `ink build --check` twice,
  each with an empty cache of its own, and fails on any error or warning (so on a broken or unpublished link),
  on a native build that differs from the components' build, and on two builds that make different deploys.
  The stage reads the counts from the JSON of `--check`, as it reads the deploys, and the diagnostics go to
  stderr. The `sites` job lists `docs/**` as an input.
- **The fixtures were called wrong.** The stage passed each fixture as a positional folder, which `ink` no
  longer takes since the global `-C` (0061); it now passes `-C`.
- **Gaps are written, not worked around.** The theme has no tokens, so the site is `theme-quiet` as it is
  and the token file does not reach it; there is no sidebar, previous and next, or outline; a dated page is
  a post. Each is in the roadmap's M5 list and none is hidden with extra CSS or front matter.
