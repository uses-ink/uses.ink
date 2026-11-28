---
number: "0171"
title: "The landing page is an `ink` site in `landing/`, wearing the docs look, with its picture drawn by `cargo xtask brand`"
status: built
date: 2026-11-28
area: release
supersedes: []
---

## 0171 The landing page is an `ink` site in `landing/`, wearing the docs look, with its picture drawn by `cargo xtask brand`

Built (2026-11-28), the landing item of M5. It replaces the roadmap's plan to serve the landing from the
`web` crate with the pen as a wasm module: the owner asked for a static page with no JavaScript, and a
site the product builds is the second dogfood after the docs (0120).

- **The site is the folder.** `landing/` holds `ink.toml`, `index.md` and `scene.svg`. The theme is
  `ink:theme-quiet` with `look = "uses-ink"` (0142); there is no second theme and no stylesheet of its own.
  Everything on the page is a markdown element the engine already renders (0027): headings, a table, code,
  a quote, one image with its alternative text. The page has no script and no form, and it asks for no font file.
- **The mock is the source of the words and the picture, not of the page.** The doodle mock (a local
  file, not tracked) draws every stroke in the browser and reacts to what is typed; neither
  survives a page that is HTML and CSS only, and the markdown format allows no `<svg>`, `<section>` or
  `<form>` (0027). What carries over is the headline, the five sections, the copy, and the scene of the first
  panel, redrawn as `scene.svg` by the freehand pen from `brand/figure.toml` (a drawing may now name a
  `folder`, so a site holds its own picture; `brand` is the default). Its colours are token names.
- **No claim the repository cannot back.** The words are the mock's, cut where the product does not do it
  yet: the Obsidian plugin and browser upload (open, Q18), a garden layout, `drafts = [...]` in `ink.toml`
  (the key is `exclude`; a `drafts/` folder is already skipped by inference), "the same features" of a
  self-host, and every claim of users. The page says it is pre-release and that the hosted service is not
  open. There is no publish form, since there is nothing to publish to; the one command shown is
  `ink preview`, which exists. Links go to the repository, and not to a sign-in or a status page, which
  do not exist.
- **The look is the docs look.** A doodle look (Gaegu, black pencil, bucket fills) needs a second look in
  `token.toml` and a theme that draws more than the docs layout; that is the owner's call on the
  rebrand (PRODUCT.md says the identity is being redone). Left as a gap, not worked around with CSS.
- **Checked like the docs.** `cargo xtask check sites` builds `landing/` with `ink build --check` twice, each
  with an empty cache, and fails on any diagnostic, on a native build that differs from the components' build,
  and on two different deploys. The stage keeps one list of such sites, `docs` and `landing`.

Left: deploying it to `uses.ink` and the domain, which are the owner's; a doodle look and the hero panel's
layout; the address field of the mock, once there is a service behind it.

Amended by 0180 and 0181: the doodle look and the hero panel are built; the page is a page of panels, not the
docs look, and the pictures of the mock are drawn by the pen. The "no form" and "no second look" lines above no
longer hold.
