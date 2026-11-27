---
number: "0018"
title: "One config file, `ink.toml`; the decision code is the pin path"
status: accepted
date: 2026-11-26
area: config
supersedes: []
---

## 0018 One config file, `ink.toml`; the decision code is the pin path

Accepted. One optional file at the source root. Every key pins a decision or chooses a slot
plugin; the key path is the decision code. Unknown keys are diagnostics. The dashboard holds no
interpretation settings. **Why:** one source of truth, in the author's source.
