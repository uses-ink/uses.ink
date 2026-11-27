---
number: "0014"
title: "Theme contract: the engine owns structure, themes render pages"
status: accepted
date: 2026-11-26
area: theme
supersedes: []
---

## 0014 Theme contract: the engine owns structure, themes render pages

Accepted, owner. The engine owns routes, feeds, sitemap, images, CSP and headers. A theme
exports `describe`, `assets`, `render` per page, and may declare extra routes with `routes` and
`render-route`; collisions are diagnostics. **Why:** every theme gets correct feeds, links and
security headers, caching stays per page, and themes still have room for unusual pages.
**Reversal:** a real theme need that cannot be met by extra routes.
