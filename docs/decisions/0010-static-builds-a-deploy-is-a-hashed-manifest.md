---
number: "0010"
title: "Static builds; a deploy is a hashed manifest"
status: accepted
date: 2026-11-26
area: engine
supersedes: []
---

## 0010 Static builds; a deploy is a hashed manifest

Accepted. Nothing renders on the reader path. Publish and rollback are pointer writes. **Why:**
atomic publish, trivial rollback, edge-cacheable reads. **Reversal:** per-reader content
(members, private posts behind auth), not in scope.
