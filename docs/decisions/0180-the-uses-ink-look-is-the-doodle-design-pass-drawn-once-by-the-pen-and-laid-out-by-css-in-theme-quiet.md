---
number: "0180"
title: "The uses.ink look is the doodle design pass, drawn once by the pen and laid out by CSS in `theme-quiet`"
status: built
date: 2026-11-28
area: theme
supersedes: []
---

## 0180 The uses.ink look is the doodle design pass, drawn once by the pen and laid out by CSS in `theme-quiet`

Built (2026-11-28). The owner looked at the docs site and the landing and said neither looks like the
design pass. They were right: 0142 filled `[look.uses-ink]` with a warm cream paper, a near-black ink and a
Literata page, a palette it called "restrained and meant to be replaced", and nothing in the design pass
(the doodle mocks, a local folder that is not tracked) is cream. This decision replaces that look and amends
0142, 0016 and 0171. The spec is the mock's direction contract, its `index.html` and its reader (`reader.css`,
`post.html`, `marks/`).

- **The look.** Pure white paper, pure black pencil, the red pen `#e8170b` for uses.ink speaking and for
  the mark under a link, the bucket yellow `#ffff00` for the one thing to click. Roles of
  `[look.uses-ink.light]`: ink `#000000`, paper `#ffffff`, muted `#5c5c5c`, line `#000000`, tint `#ffffff`,
  accent `#e8170b`. The paint box of `[colour]` is unchanged (it already matched the mock), and every
  look's stylesheet now also declares the colours that are no role (`--pen`, `--pencil`, `--bucket`,
  `--sky`, `--mint`, `--pink`, `--cloud`) and a third face, `--face-hand`. A role is never renamed: the
  dashboard reads `[colour]` and `[face]` and keeps working.
- **No dark scheme.** The mock shows none: its paper is white and the fills (yellow, sky, mint, pink) are
  paint-box colours that mean something only on white. `dark` is now optional in a look; a look without one
  keeps its paper in a dark setting, and the stylesheet says `color-scheme: light`. The dark overrides of
  `code.css` would put light syntax colours on white paper, so the look sets the code custom properties itself
  (the pencil, the pen for keywords and deletions, the muted grey for comments).
- **Contrast.** `crates/core/token/tests/contrast.rs` still holds ink, muted and accent to 4.5:1 on paper
  and on tint. Red on white is 4.61:1, so the pen passes AA with no exception; muted on white is 6.69:1. The
  pen is never set on a fill (it is 4.29:1 on the yellow): every text on a fill is the pencil, 19.56:1 on
  yellow, and a new test checks the pencil on all four fills. Colour never carries state alone: a ticked
  item is a checked checkbox in the markup, a current page carries `aria-current` and the drawn underline.
- **Faces.** The reader mock (`reader.css`) sets post text in Literata and names, headings, navigation,
  captions and notes in Gaegu Bold, so the reader pages do the same, with Monaspace Radon for code. The
  landing mock sets everything in Gaegu (400 for text, 700 for headings and notes). The look gained a `hand`
  face beside `body` and `mono`; sizes follow the mock (body 1.25rem, h1 3.875rem, h2 2.125rem).
- **Marks are drawn at build time, not in the browser.** The mock draws through `pen.js` and the wasm pen of
  0034 in the browser; reader pages have no JavaScript (0020) and the owner asked for a static landing
  (0171), and the reader mock itself says "every mark is a pre-drawn SVG". So `cargo xtask brand`
  draws the marks once from `brand/figure.toml` (0025, 0033) into
  `crates/runtime/plugin/theme/quiet/assets/marks/`: `rule`, `underline-ink`, `underline-pen`, `highlight`,
  `mint`, `tape`, `bullet`, `circle`, `frame`, `arrow-down`, `note-arrow` and `tick`. The theme serves them as
  its own files (`/marks/<name>.svg`) and the stylesheet lays them out as backgrounds and `border-image`s.
  `border-image` keeps the frame's stroke at one width round any box; a stretched background would not.
  A drawing may now say `stretch`, which writes `preserveAspectRatio="none"` for a mark laid along a line.
  There is nothing to degrade: the page that ships is the CSS and the static SVG, and it is the drawn page.
  The browser pen stays for the dashboard.
- **Fonts.** The media types a site serves have `woff2` and no TTF, so the faces are woff2 under
  `crates/runtime/plugin/theme/quiet/assets/fonts/`, served at lowercase routes (`/fonts/gaegu-bold.woff2`; a
  route is lowercase). The Gaegu TTFs are 3 MB each; `python3 -m fontTools.subset <ttf> --unicodes=<Latin,
  Latin-1, Latin Extended-A, general punctuation, arrows> --flavor=woff2 --layout-features='*'` makes them
  17 and 23 KB. The two Literata variable fonts were first pinned with `python3 -m fontTools.varLib.instancer
  <ttf> opsz=14 wght=400:700` and then subset the same way, 43 KB each. All are SIL OFL 1.1 (Gaegu and
  Literata carry no reserved font name, so a subset is allowed). Monaspace Radon has the reserved name
  "Monaspace", so its woff2 is shipped as released, unmodified. Each family's licence text, with its own
  copyright line, is beside the fonts (`OFL-*.txt`); the folder's `OFL.txt` in the mock is Shantell Sans's and
  was not used.
- **Where it lives.** A look of `theme-quiet`, as 0142 already had it, not a second theme: 0016 named
  `ink:theme-notebook` for the hand-drawn one, but a second theme would copy the document writer for the
  sake of a stylesheet and a few classes. `look = "uses-ink"` links `/look.css` (the custom properties,
  generated) and `/notebook.css` (`assets/notebook.css`, plain CSS, after the `hand` classes the markup uses).
  The `quiet` look is byte for byte what it was: `every-node.snap` did not move. The drawn look adds to the
  markup only a footer (the feed when there is one, and "made with uses.ink", the one line uses.ink signs)
  and, on a page of panels (0181), the logo and the panels.
- **What the reader pages follow.** The header (the name highlighted once, over a drawn rule), the title and
  meta line, h2 with a drawn underline, red pen underlines on links with the highlighter swiping in on
  hover, drawn bullets, framed and taped pictures, the quote with a red quote mark, code in a drawn frame, the
  home as a list by date with red dates, and the footer. The docs navigation is in 0182.

What still differs from the mock, and why:

- **Margin notes.** The mock turns a footnote into a red note in the margin (a checkbox toggle on a phone).
  That is a markup change (a footnote's text inside its reference), not a stylesheet, and a footnote is
  blocks, which cannot sit inside a paragraph. The footnotes are still listed at the end. Left.
- **Every stroke is the same stroke.** The mock seeds each element, so two boxes differ; a shipped frame is one
  drawing stretched, so two boxes of one size are alike and a very long or very short box shows the stroke
  thinner or thicker than at the drawing's own size. No pencil boil on hover and no stop-motion draw-in: those
  are redraws and need a script.
- **Fills.** The mock steps every bucket fill on a 3px grid, offset 5px right and 4px down. Here a fill is a
  drawn patch placed with the same 5px and 4px offset, not stepped.
- **Post facts the engine does not have.** The mock's meta line says "4 minute read" and "from
  notes/shelves.md"; the page shows its date and tags.
- **Not seen in a browser.** No farm host has a browser and none runs here, so the stylesheet was written
  against the mock's CSS and the built HTML and checked by structure, not by sight.
