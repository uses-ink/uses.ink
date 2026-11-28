---
number: "0140"
title: "A `[folder]` table in `ink.toml` rules the kind, weight and order of the pages in a folder"
status: built
date: 2026-11-28
area: config
supersedes: []
---

## 0140 A `[folder]` table in `ink.toml` rules the kind, weight and order of the pages in a folder

Built (2026-11-28). The docs site had two faults that no page could fix alone: every decision has a
`date`, so each was a post and the home listed them under "Posts"; and a generated home listed pages by
route with no way to put one first.

- **The table.** `[folder."docs/decisions"]` takes `kind` (`page` or `post`), `weight` (a whole number,
  default 0) and `order` (`route`, `title`, `newest` or `oldest`, default `route`). The key is a path in
  the snapshot, so it is the same path `ink select` prints. An unknown key, a kind other than the two, a
  weight that is not an `i32` and a table that is not a table are `config.*` diagnostics at the written
  place.
- **One more rung for `page.kind`.** The ladder is now `pin`, `front`, the folder table, the `Posts`
  folder rule, a date in the front matter, a date in the file name, then the fallback. The deepest folder
  that holds the file and says a kind decides, and a folder is not inside itself. A page's own `kind` in
  its front matter still wins, and `ink explain --why` shows the new rung (`folder table`) with its span.
  Every explain golden of `page.kind` gains the rung, which is empty unless a site writes a table.
- **`weight` and `order` are not decisions.** They do not change what a page is, only where it is listed,
  so they carry no decision code and no count of decisions moves. They reach the model as
  `Site.sections`, part of every page's render key, so editing a weight rebuilds the pages that show it.
- **One function of the site.** `Site::contents()` is the tree of folders in reading order and
  `Site::neighbours(page)` the page before and after. Both are pure functions of the `Site`, which is what
  a theme in a component receives, so a native theme and a component write the same list. The top lists
  the `site.nav` pages first in nav order, then the rest by route; a folder's `index` or `README` stands
  for the folder; folders sort by weight and then name. The home and `/404` are not in it.
- **The docs site** sets `[folder."decisions"] kind = "page"`, so the decisions are pages, not posts, and
  the "Posts" listing and the feeds it brought are gone from the docs home.

Left: a per-page `weight` in front matter. Folder level answers the docs site and a page can move by being
renamed.
