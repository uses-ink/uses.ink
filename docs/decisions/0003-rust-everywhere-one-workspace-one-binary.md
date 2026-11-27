---
number: "0003"
title: "Rust everywhere, one workspace, one binary"
status: accepted
date: 2026-11-26
area: repo
supersedes: []
---

## 0003 Rust everywhere, one workspace, one binary

Accepted. Engine, control plane, edge, builder and CLI are Rust in one workspace; the binary is
`ink`. **Why:** the control plane would otherwise exist twice (Worker and self-host); shared
types without codegen. **Reversal:** if workers-rs blocks the edge, only the edge adapter moves
to a thin TS Worker over the Rust control plane compiled to wasm.
