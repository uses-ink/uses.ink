---
number: "0030"
title: "The `Db` port is shaped like D1"
status: accepted
date: 2026-11-26
area: data
supersedes: []
---

## 0030 The `Db` port is shaped like D1

Accepted. Queries and atomic batches of raw SQL, no interactive transactions, one migration set
in wrangler's format with our own runner for SQLite. **Why:** D1 is the stricter target; a port
shaped like SQLite would let self-host code do what hosted cannot. **Reversal:** D1 gains
interactive transactions.
