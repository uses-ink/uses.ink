---
number: "0190"
title: "The dashboard is the doodle binder: its boxes are drawn by the server as border images, and it serves its own faces"
status: built
date: 2026-11-28
area: web
supersedes: []
---

## 0190 The dashboard is the doodle binder: its boxes are drawn by the server as border images, and it serves its own faces

Built (2026-11-28). The dashboard did not look like the design pass (`.impeccable/mocks/doodle/dashboard`) for
three reasons, each fixed here. No face reached the page (the sheet named Gaegu and Monaspace Radon, nothing served
them, and the policy would have blocked them), the page was one centred column with a row of links where the mocks
have a binder (tabs down the left, one boxed sheet), and every box was a CSS `3px solid` rectangle where the mocks
have four crossing pencil strokes with a bucket fill that misses its lines.

- **The layout is the mocks'.** `shell.rs` draws a rail (the mark, `your site <handle>`, one tab per screen, and the
  person's own tab with a face at the bottom) and `main` as the sheet. The tabs wear the mocks' fills in order: site
  yellow, what we understood pink, deploys sky, settings mint. The current tab reaches over the sheet's edge. Under
  980 px the rail is a row of tabs above the sheet and Account stays in it. Headings are lowercase by
  `text-transform`, so the words in the markup and the tests stay as written.
- **Boxes are drawn by `freehand` and laid as border images.** `/outline-1.svg`, `-2` and `-3` are one large frame
  each (480 px square, four lines whose corners miss by up to 3 px, one 3 px pencil), drawn when asked for from a fixed
  seed, so the file is the same every time and no generated drawing is committed (0025). The stylesheet puts one on an
  element's `::after` with `border-image-slice: 12`: the corners stay as drawn and the sides stretch along their own
  length, so a line keeps one width at any size and keeps its wobble. The same pseudo-element is a plain black 3 px
  box where the image does not load, so the page without the image is already a drawn page in the mocks' layout. A
  fill is the element's `::before`, moved 5 px right and 4 px down. Three hands alternate by `:nth-child`, so
  neighbours do not wobble alike. Inputs wear the image as their own border, since they hold no pseudo-element.
  `/rule.svg` runs under each table row, `/underline.svg` is the red link underline, `/face.svg` the person's tab.
- **The faces are subset and served.** `crates/surface/web/assets/` holds Gaegu regular and bold and Monaspace Radon
  regular as `woff2`, cut from the full fonts of the mocks (3 MB each for Gaegu, which carries Hangul) to Basic Latin,
  Latin-1, the general punctuation and a few symbols: 17, 23 and 49 KB. Made with
  `python3 -m fontTools.subset <font> --unicodes=U+0020-007E,U+00A0-00FF,U+0131,U+0152-0153,U+2010-2027,U+2030-203A,U+20AC,U+2190-2193,U+2713,U+2022 --flavor=woff2 --layout-features='*'`.
  `OFL.txt` beside them is the licence. A person whose name or content needs a glyph the subset lacks gets the face's
  fallback (`system-ui`) for that glyph.
- **The colours and stacks are `token.css`.** `/token.css` is `crates/core/token/token.css` as generated, so the
  dashboard reads `var(--ink)`, `var(--hand)` and the rest, and a change to `[colour]` or `[face]` in `token.toml`
  reaches it with no edit here. The sheet names no colour of its own.
- **Files are one typed value.** `Asset` (`asset.rs`) lists every file, its path, its medium and its bytes; the route
  `/<file>` answers it before the handle arm, a name with a dot being no handle. The API host sends them with
  `cache-control: public, max-age=86400` in place of the pages' `no-store`; `edge-api`'s `Media` gains `Font`, `Svg`
  and `Css`.
- **The policy moves by three words.** `font-src 'self'`, and `style-src 'self' 'unsafe-inline'` for `/token.css`;
  `img-src 'self'` was there. `data:` stays out, which is why the strokes are files and not inlined: a border image
  from a `data:` URI would need `img-src data:`. Script is unchanged (`/passkey.js` and nothing else, 0053).
- **The pen is not shipped to the browser.** 0034 allows `pen.js` and `freehand.wasm` on the dashboard, and the
  mocks use them to draw each box at its measured size. This does not. 0053 keeps script off the dashboard unless a
  decision says a screen needs it, and none does: the sheet has to look right without it, so the server draws what it
  can, and a client pen on top would need `'wasm-unsafe-eval'` in the policy and a page that is half drawn while the
  module loads. Its cost: the wobble along a side is a stretch of one drawing, not a new drawing per element, so two
  boxes of one size and one hand are identical, and a fill is a plain offset rectangle where the mocks' is a
  ragged patch.
- **Red is the red pen only where it reads.** Text in `#e8170b` is 4.6:1 on paper and fails on yellow, mint, sky and
  pink, so the sheet sets it only on paper: the reasons uses.ink gives (`why`, the guess to check, the note, a
  failed build), the ring around a problem, the failure heading. Ink is at least 4.5:1 on every fill, pencil on
  paper, cloud, yellow, mint and sky. Focus is a dashed red outline 4 px off the element, so it sits on paper. A
  test holds each pair (`tests/pages/look.rs`).
- **Existing screens keep their words and structure.** Account, members, step-up, sessions, recovery, abuse, the
  repository and domain forms, sign-in and the failure pages are the same markup in the same components, restyled.
  A domain or a repository waiting for proof shows what to publish as the mocks' record box.

**Not in the mocks and not here:** the deploy filters and the copy buttons (they need script, and the pins are
selectable `<code>`), "add a source" (no such action exists), the stick figure with a flag (the brand mark stands in),
and per-element drawn rings and underlines (a CSS ring and one drawn underline file). A table is a scroll region at
phone width, as 0053 says, where the mocks stack its rows. What the mocks show that the view models do not hold (a
deploy's branch and commit, a page count, a build time) is not shown; adding it is a change to the models.

**Why:** the dashboard is the part people look at most and the owner drew it; a page that loads none of the drawn
look is not the design. **Reversal:** if the strokes need to be unique per element, ship the pen as 0034 describes
for the pages that want it, behind this sheet, which is already the fallback.
