---
number: "0182"
title: "The docs navigation is drawn in the same hand, with yellow only where a link is pointed at"
status: built
date: 2026-11-28
area: theme
supersedes: []
---

## 0182 The docs navigation is drawn in the same hand, with yellow only where a link is pointed at

Built (2026-11-28), on top of 0141 and 0180. The sidebar, the outline and previous and next are the same
`nav` elements as before, so the markup, the landmarks and the order did not change; only the sheet of the
drawn look did.

- **Contents.** Gaegu Bold. Each folder's `summary` has a drawn bullet; the current page keeps
  `aria-current="page"` and is drawn with the ink underline mark and bold, so it is not marked by colour
  alone. A link under the pointer gets the same underline. No box, no fill.
- **On this page.** The rubric is red, since it is uses.ink speaking; the entries are pencil, with the red
  pen underline under the pointer.
- **Previous and next.** Each link is a framed box (`border-image` of the drawn frame), the hint (`Previous`,
  `Next`) in red, and the yellow highlighter fills the box under the pointer. The yellow of the look marks the
  one thing to click, and a page has only the pointed-at link in yellow at a time.
- **Narrow screens** are as in 0141: the three stack in reading order.
- **The docs site** is `look = "uses-ink"` and `navigation = "docs"` as before; what changed is the sheet it
  links. `docs/site.md` says how it is built and what it lacks.
