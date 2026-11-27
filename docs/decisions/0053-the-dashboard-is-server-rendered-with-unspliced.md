---
number: "0053"
title: "The dashboard is server-rendered with `unspliced`; no Datastar, no script; the log is a streamed page"
status: accepted
date: 2026-11-26
area: web
supersedes: []
---

## 0053 The dashboard is server-rendered with `unspliced`; no Datastar, no script; the log is a streamed page

**Decision.** `web` is the dashboard (Site, What we understood, Deploys, Settings, Account) as
typed view models and pure rendering. A page is a function of a view model to `unspliced` values;
the stylesheet is `css!` over the tokens (`token/token.toml`), and the pen's marks are drawn at
render time by `freehand`. Nothing is spliced, and no JavaScript ships: not Datastar, not maud,
not htmx (the frontend research recommended maud plus Datastar; the owner rejected both).

- **Interaction is native HTML.** Every change is a full-page form posted to `/<handle>/<action>`
  and answered by `303` to the page it changed (post-redirect-get). Open and close and "show why"
  are `<details>`; filtering is a link with a query. A running build shows `<meta
  http-equiv="refresh">` on the Site and Deploys screens, and only while one runs.
- **The live log is a streamed page.** `Split` cuts the page at one mark inside the log's
  container; the edge sends the head, one chunk per `build::Event`, and a last chunk holding the
  result (published, refused or failed, with the fix) or, if the log stops early, a typed
  `build.log.cut` failure. A finished build is one ordinary page from the same line renderer. If a
  screen ever truly needs script, it is said in a decision here first, not added.
- **`web` depends on no HTTP server.** It holds the view models, `Route`, `Action`, `Failure` and
  one trait, `Screens`, that the edge implements over `control` (every method answers a view model
  or a `Failure`; a build's events come from `Follow`). `Dashboard::respond` turns a `Request` into
  a `Reply` (status, location, a page, a stream or a download); the edge maps it to its HTTP
  library and adds `Reply::HEADERS` (no-store, a CSP with no script, `form-action 'self'`).
- **Every failure is a page with a code.** `Failure` has a code (`site.handle.unknown`,
  `form.field.invalid`, `build.log.cut`, ...), a status, a cause chain kept as a chain and a fix.
  A control-plane `Error` converts to it; the one erasure of a port's error into a `Cause` is
  there.
- **Words are written once.** The wording of a decision's value, its evidence and its pin
  moved from the CLI into `diag` (`Value`, `Because`, `Pin`), so `ink explain` and the dashboard
  say the same thing.
- **Accessibility is in the sheet and checked in the tests**: visible focus (3 px outline and a
  fill), state as a word plus a glyph and never colour alone, `prefers-reduced-motion` guards the
  one animation, a mobile layout under 40 rem with tables in focusable scroll regions, a skip link,
  one `h1` a page, a name on every form and a label on every control.
- **`role` in `unspliced`.** The vocabulary lacked the `role` attribute (ARIA defines it, HTML's
  index does not list it); the generator now reads ARIA's concrete roles and webref's definition
  of the attribute.

**Not done:** the pen through wasm (the landing's), per-branch previews and a decisions diff
between deploys (the control plane has neither), custom domains and linked identities (it holds no
rows for them; their screens say what to run), sign-in, and the edge's mounting of `Screens`.
**Why:** the dashboard is the part people look at most and the part that most invites a framework;
five screens of server state and one log do not need one, and the owner's crate already streams.
**Reversal:** if a screen needs client state (reordering, an editor), add an island for that
screen alone as a decision here, not a framework for all.
