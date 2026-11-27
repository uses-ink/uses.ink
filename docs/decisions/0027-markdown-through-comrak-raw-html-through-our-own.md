---
number: "0027"
title: "Markdown through comrak; raw HTML through our own allowlist"
status: accepted
date: 2026-11-26
area: format
supersedes: []
---

## 0027 Markdown through comrak; raw HTML through our own allowlist

Accepted. comrak is the only parser with nodes for wikilinks, embeds, callouts and block ids,
with spans. Raw HTML is parsed into `Document` against our allowlist; ammonia only behind an
escape hatch. **Why:** pulldown-cmark had two 2026 wikilink advisories; ammonia had four mXSS
advisories in 2026 (`research/deps-engine.md`).

The allowlist, as the markdown format reads it: `<kbd>`, `<sup>`, `<sub>`, `<mark>`, `<del>` and
`<s>`, `<br>`, `<img src alt>` and `<a href>` inline; `<details>` with an optional first
`<summary>` around markdown, `<hr>` and `<br>` as blocks. No attribute is kept beyond `src`, `alt`
and `href`, which follow the same rules as markdown images and links. Every other element is
removed with `html.element.stripped` and its text kept.
