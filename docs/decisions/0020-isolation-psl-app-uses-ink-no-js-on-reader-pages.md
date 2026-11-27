---
number: "0020"
title: "Isolation: PSL, `app.uses.ink`, no JS on reader pages"
status: accepted
date: 2026-11-26
area: security
supersedes: []
---

## 0020 Isolation: PSL, `app.uses.ink`, no JS on reader pages

Accepted. `uses.ink` goes on the Public Suffix List before launch; the dashboard is its own site;
reader pages have no JavaScript, no cookies and a strict CSP written by the engine.
