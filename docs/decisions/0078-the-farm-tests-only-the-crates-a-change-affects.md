---
number: "0078"
title: "The farm tests only the crates a change affects, when asked"
status: accepted
date: 2026-11-26
area: farm
supersedes: []
---

## 0078 The farm tests only the crates a change affects, when asked

Accepted, owner. A second measurement of 100 agent transcripts showed 278 `farm cargo nextest`
calls taking 2.5 hours and 372 full `farm check` runs taking 4.3 hours, with agents running the
whole workspace's tests for a change in two crates. Extends 0047 and 0075.

- **`farm test` and `farm check <job> --affected` are explicit.** The full `farm check` stays the
  gate, and `--affected` is never applied to it or to a cached full stage. A job opts in with
  `affected = true`; `--affected` on any other is a usage error.
- **The change is what jj says.** `jj diff --summary` from a base to the working copy; the base is
  `--base`, else `[affected] base`, else the parent of the workspace's first commit above `main`.
  `--summary` and not `--name-only` because a rename must count its old path, which only the
  summary prints. jj sits behind a trait, so no test runs it.
- **Crates come from `cargo metadata --no-deps`, run on a host in the synced workspace.** The
  farm stays std, serde, serde_json and toml, and reads no project path. The members' manifest
  directories give ownership by longest prefix; a member's dependency list names the workspace
  crates it uses (normal, dev and build), which gives the edges without a full resolve and without
  the network. The call is a job like any other, so it takes a slot on the host placement picks.
- **When in doubt, all.** A root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`,
  `.config/**` or `.cargo/**` affects every crate, and so does a path in no crate (any test may
  read a fixture at the root) unless the repo lists it in `[affected] ignore`. Only a change of
  ignored paths, such as prose, is `no crate affected`: no job, exit 0.
- **The run is `cargo nextest run -p a -p b ...`** (`--workspace` for all), on a golden host, with
  the person's nextest arguments after it, and one line says what was chosen and why.
- **What it cannot see** is an edge that is not in a manifest: a test that reads a sibling crate's
  source, a build script reading a file. The one full gate at the end covers those.
