---
number: "0008"
title: "`git.fetch` is a host capability"
status: accepted
date: 2026-11-26
area: plugins
supersedes: []
---

## 0008 `git.fetch` is a host capability

Accepted. Git runs host-side with gix; the git source plugin maps it to a snapshot. **Why:** it is
the one operation that needs a real disk and a native library.
