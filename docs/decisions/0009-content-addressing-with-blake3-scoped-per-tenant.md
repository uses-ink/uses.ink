---
number: "0009"
title: "Content addressing with BLAKE3, scoped per tenant"
status: accepted
date: 2026-11-26
area: data
supersedes: []
---

## 0009 Content addressing with BLAKE3, scoped per tenant

Accepted. Every blob is keyed by its BLAKE3 hash under its tenant's prefix; no cross-tenant
dedup. **Why:** exact deletion, no existence leak through dedup timing.
