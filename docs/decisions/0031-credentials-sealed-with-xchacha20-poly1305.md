---
number: "0031"
title: "Credentials sealed with XChaCha20-Poly1305"
status: accepted
date: 2026-11-26
area: security
supersedes: []
---

## 0031 Credentials sealed with XChaCha20-Poly1305

Accepted. Sealed per row with the row id as associated data, so a copied row fails to open. This
replaces the AES-256-GCM suggestion in `research/architecture.md`. **Why:** a 24-byte random nonce
needs no counter; `age` and `hpke` cannot share a build today.
