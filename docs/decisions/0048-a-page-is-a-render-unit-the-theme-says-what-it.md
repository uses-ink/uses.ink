## 0048 A page is a render unit; the theme says what it reads

Accepted. Incremental builds key each page by the hash of the engine's release, the theme's
identity (a component's hash, its tokens), the base address, the page, and the part of the site
the theme says rendering reads: `Theme::reads(page)` is `Linked` (identity, navigation, linked
pages without bodies, the assets shown), `Listed` (also every page without its body) or `Whole`,
the default. A hit is served without asking the theme. A theme with no `identity` keeps no page.
The feeds, sitemap and `robots.txt` are made every build. This refines 0006: a theme is handed
the whole site and a read through it cannot be observed, so the theme declares what it reads,
and the tests hold a declaration to what the theme does (after any one edit, an incremental
build gives the cold build's bytes, and an untouched page is not rendered again).
**Why:** the declaration is the cheap sound option; recording reads would need a lazy site
across the component boundary (0041). **Reversal:** a theme's `reads` is wrong more than once
in a release: render every page every build and key only the assets.
