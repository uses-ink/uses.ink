---
number: "0141"
title: "The theme gets the section tree, the neighbours and the outline, and `theme-quiet` draws them with `navigation = \"docs\"`"
status: built
date: 2026-11-28
area: theme
supersedes: []
---

## 0141 The theme gets the section tree, the neighbours and the outline, and `theme-quiet` draws them with `navigation = "docs"`

Built (2026-11-28), on top of 0140.

- **Data through the API a theme has.** A theme already receives the whole `Site` and the page. The tree
  is `Site::contents()`, the neighbours `Site::neighbours(page)` and the outline `Document::outline()`
  (headings of level 2 and below, with their anchors and plain text, none from inside an embed or a
  footnote). The WIT `site` record gained `sections: list<section>` and a `section` record with its
  `order` enum, mirrored in `wit/mirror/site.rs`; nothing else about the theme's data changed.
- **The ABI version.** There is no precedent for a bump: every WIT package is `@0.1.0` and a plugin's
  version is its crate version in the `bundle.json` that `cargo xtask component` writes. The theme world
  changed shape (0142), so the bundle's version is what tells a host a component is newer; the package
  version stays `0.1.0` until a release says otherwise.
- **Opt in.** `[theme] navigation = "docs"` turns on the chrome; the default is `header`, so every site
  without the token renders the same HTML as before. The layout cannot decide it, since the docs site is
  built from a folder and has layout `flat`.
- **What is drawn.** `nav` labelled "Contents": each folder is a `details`, open for the folder that holds
  the page, and the current page is a link with `aria-current="page"`. `nav` labelled "On this page": the
  headings of the page, when there are two or more below the title, each a fragment link. `nav` labelled
  "Previous and next". No script. Below 64rem the three stack in reading order; above it the sidebar and
  the outline are sticky columns on either side of the page.
- **Cache.** With the chrome on, every page shows every other page's title, so `Theme::reads` is `Listed`
  for all of them and a page's key follows the fronts of the site. The default stays `Linked`.
- **A generated home** lists pages in `contents()` order, not route order, with and without the chrome.
  The home of a site that writes its own is unchanged.

Fixtures: the quiet theme's `style.css` is now written with custom properties (`--face-body`, `--size-h1`
and the like) whose defaults are the old sizes, so its bytes changed and the deploy id of every site
fixture with it. Their page HTML did not change. Goldens were blessed through `farm bless` and the diffs
read: only the `deploy` field of each build result moved.

Left: search; a mobile menu that folds the sidebar (the stacked layout is the answer for now).
