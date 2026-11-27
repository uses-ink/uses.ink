---
number: "0022"
title: "Deletion removes bytes before rows"
status: accepted
date: 2026-11-26
area: control
supersedes: []
---

## 0022 Deletion removes bytes before rows

Accepted. Unpublish, delete blobs under the tenant, delete rows, send a receipt. A crash leaves a
diagnosable row, never orphaned bytes.
