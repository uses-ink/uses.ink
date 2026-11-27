---
number: "0006"
title: "The engine is pure"
status: accepted
date: 2026-11-26
area: engine
supersedes: []
---

## 0006 The engine is pure

Accepted. The engine has no I/O; it asks host traits for everything, and every read is recorded
and becomes part of a render unit's cache key. **Why:** determinism, incremental builds without
declared dependencies, the same engine natively and in wasm.
