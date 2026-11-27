---
number: "0032"
title: "HTML, SVG and CSS through `unspliced`; no maud, no Datastar"
status: accepted
date: 2026-11-26
area: web
supersedes: []
---

## 0032 HTML, SVG and CSS through `unspliced`; no maud, no Datastar

Accepted, owner. Every page uses.ink renders (themes, dashboard, landing) is written with
`unspliced`, a crate of its own (`~/code/personal/unspliced`, MIT OR Apache-2.0). It rebuilds the
owner's earlier `press` (stik) and `vun-html` (vun) as one library: names are typed constants,
documents are baked by the compiler, text escapes itself, URLs from run time are checked, and
there is no JavaScript runtime: behaviour is native HTML (`popover`, `<details>`, `<dialog>` with
`command`), forms that post and redirect, cross-document view transitions, and streamed pages
(`Split`) for live build logs. A typed action runtime was built and dropped on 2026-11-26 as a
private framework the platform no longer needs. **Why:** maud lets typos compile and leaves CSS and JS untyped;
Datastar puts untyped expressions in attributes and needs `unsafe-eval` or a nonce. **Reversal:**
none planned; Leptos remains the documented fallback for the dashboard.
