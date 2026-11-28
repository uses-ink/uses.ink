---
number: "0181"
title: "A page of panels is markdown cut at its rules, and the landing is one"
status: built
date: 2026-11-28
area: theme
supersedes: []
---

## 0181 A page of panels is markdown cut at its rules, and the landing is one

Built (2026-11-28). 0171 made the landing an `ink` site and said the doodle mock's first panel, address
field and button could not be markdown. They cannot be raw HTML, since the format strips `svg`, `section` and
`form` (0027), and a folder source serves no `.html`, `.css` or font of an author's. The answer is a token and a
few rules in the theme. There is no second renderer, so the landing is still markdown, still built twice by
`check sites`, and every rule is written here.

- **The token.** `[theme] layout = "panels"` (default `page`). `landing/ink.toml` sets it with
  `look = "uses-ink"`.
- **Cut at rules.** The blocks between two thematic breaks (`---`) are a `section`. The first is the hero. The
  blocks after the last rule are the page's foot, in a `footer`, and a page of panels has no footer of the
  theme's. The rule itself is not drawn. The header gets the logo before the name.
- **Two voices.** A paragraph that is all emphasis is uses.ink speaking, in the red pen (`*no config file, no
  build step. promise!*`). A paragraph that is one link, after an optional code span, is the page's action: a
  form whose field shows the code span as its example and whose yellow button is the link, going to the
  link's address. No other paragraph changes.
- **Lists of pictures.** A list whose items hold a picture is a row of cards: five columns in a tight list
  of notes, a picture and a code name (what uses.ink understood); three in a tight list with a bold name
  (the sources); two framed cards in a loose list (what stays yours). The count sets the columns and a phone
  has one or two. A red emphasis before a picture gets the arrow, an emphasis after the text is a note. This
  is CSS over the markup the theme already wrote, with `:has`.
- **A block quote is the sticky note** (yellow, tilted, with the one thing to write), **a task list is the
  drawn ticks**, and **a callout is the browser window** (its title is the address in the pink bar).
- **The pictures** are drawn by `cargo xtask brand` from `brand/figure.toml` into `landing/`, from the mock's
  own shapes: the scene of the first panel, the little guy, the five things found, the three sources, the two
  keeps. They are images with alternative text, so the page reads without them.
- **Words and what was cut.** The copy is the mock's where the repository backs it, and the five sections and
  their order are the mock's. Cut: the nav (`how it reads`, `sources`, `self-host`, `sign in`: the nav of a
  site is its pages, there is one, and nothing signs in), `status` in the footer, "the browser" and "the
  plugin" for a vault (open, Q18), "a garden instead of a blog" (not built), "the same features" of a self-host,
  `drafts = [...]` (the key is `exclude`), the reactive note under the field (it needs a script) and the line of
  names under the scene. The page says it is pre-release.
- **The field.** There is nothing to publish to: the hosted service is not open. The field and the button are
  there as in the mock, and the button goes to the self-host guide with the typed address as `?src=`, which the
  guide ignores; the page says so under the last form. When the service opens, the link is the one thing to
  change, and the field starts to mean what it says. This is the owner's call: the alternative is a button and
  no field.
- **Checked.** The sites stage builds `landing/` as before; the theme's tests hold the rules above, and the
  default `page` layout and the `quiet` look render as they did.

Left: deploying the page (owner), the nav once the page has places to go, and a drawn picture of the scene's
caption line.
