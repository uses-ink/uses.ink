---
number: "0081"
title: "The farm removes the copies it made, seeds from the tree used last, and stops a gate that has failed"
status: accepted
date: 2026-11-28
area: farm
supersedes: []
---

## 0081 The farm removes the copies it made, seeds from the tree used last, and stops a gate that has failed

Accepted, owner. A measurement of 100 agent transcripts (`efficiency2`) and a look at the hosts
showed five leaks in the farm's own work: copies of forgotten workspaces left on `farm-a`, first
runs in each new workspace that start cold (about 5,675 host seconds extra), `farm test` refusing
the `--affected` that `farm check test --affected` takes, siblings of a failed stage running on to
the end, and nextest timing out on hosts with many cores (19% of the runs on `farm-b` at a load
of 15, 22 timeouts costing 1.5 host hours). Extends 0047, 0075, 0078 and 0080.

- **`farm workspace --forget` removes the copy on every host.** After the local removal
  succeeds, each remote host is asked at once to remove `<dir>/<repo>/<name>`, and the command
  says per host what happened: removed (with the size), no copy, refused, or not reached. The
  host-side program takes the host's sync lock for that workspace, so a sync in progress ends
  first, and refuses unless the name is one path segment, the directory is not a symlink, and its
  resolved path is directly inside the resolved `<dir>/<repo>`, which is inside the resolved
  `<dir>`. A refusal names the resolved path and exits 4. A host that is down keeps its copy; the
  command still succeeds, because the workspace is already gone here and failing would invite a
  retry that cannot work (`Missing`), and it says to use `--prune`. `local` is never asked: it has
  no copy.
- **`farm workspace --prune` lists, and with `--yes` removes, the copies no workspace here owns.**
  Owned names are each directory of `<repo>-ws`, `main`, and the workspace the command runs in.
  Dry run is the default: removal is irreversible and the list is cheap. It sees this machine's
  workspaces only, so copies made by another checkout sharing the hosts look unowned; this is
  said in the docs and is why `--yes` is separate. Half-made `.seed` copies and directory names
  the farm would not create are not listed (a crashed seed is replaced by the next seed of that
  name).
- **A new workspace's copy is seeded from the sibling used last, among those that built.** The
  old rule took the newest directory by its own modification time, which changes when a file is
  added at its top level, so it was as often a copy that had never built, or one that had just been
  seeded, as a warm one. Now every job touches `target/` of its tree as it starts, and the seed
  takes the sibling whose `target/` was touched last and that has a `target/debug` or
  `target/release`. With none, the first build is cold and the farm says so (before, it said
  nothing). Reflink first, then a plain copy, as before.
- **Why copying the whole tree is safe, and why it is not `target/` alone.** Cargo decides what to
  rebuild from fingerprints, not from where a directory was. A workspace crate's fingerprint and
  its `-C metadata` hash are computed from the package id relative to the workspace root (cargo's
  `stable_hash`), and its dep-info records paths relative to the package or the target directory,
  so moving a tree does not invalidate them. Third-party crates come from the same `CARGO_HOME` on
  the host and are keyed by source, not by workspace path. What differs between workspaces is the
  source files, and cargo compares their modification times with the artifacts. The sync (0075,
  `--checksum --no-times`) already makes this right: after the seed, a file whose bytes changed is
  newer than every artifact and rebuilds its crate, and a file that did not change keeps the time
  it had beside its artifacts and rebuilds nothing. Seeding `target/` alone would give every source
  file the time of the first sync, newer than every artifact, so every workspace crate would
  rebuild; only the third-party crates would be saved, which is what `farm workspace` on this
  machine already gets. A wrong guess costs build time, never a wrong binary, because the sync
  and cargo's own check are the arbiters, not the seed. The root path is not baked into build
  output: `xtask` no longer reads a path from a sibling tree (the bug that made a stale copy
  matter), so a copied sibling holds no path that belongs to another workspace. A copied tree also
  carries the ignored files of its source (a build cache such as `.ink`); `--delete` of the sync
  does not remove files the `.gitignore` filter excludes. These are caches, the same as in the
  sibling, and the workspaces of one owner on a farm host share one trust. This was reasoned from
  cargo's behaviour and the existing sync test, and not yet measured: the check is a first run in
  a new workspace after this change, whose log shows `Fresh` for third-party crates and
  `Compiling` only for the crates the change touched.
- **`farm test` accepts `--affected` and `--base`.** `--base` already worked; `--affected` was a
  usage error that one agent hit. It is a no-op on `test`, which is always affected, so the two
  spellings read alike. `--keep-going`, `--prune` and `--yes` stay with their own verbs.
- **A failed stage of a full `farm check` cancels the others.** The stages of one check share a
  flag; the first to end with a nonzero code or a farm error sets it. Every call that watches it
  is killed on the client side, which makes the host stop the job exactly as for a client that
  goes away (the body notices its lease is gone and kills its process groups), so a queued stage
  leaves the queue too, and a stage that has not started never starts. A cancelled stage is its
  own verdict: `CANCELLED` in the text report, `"cancelled"` in `--json`, no exit of its own (the
  failed stage sets the exit), no timing, no journal line and never a stored pass. The farm says
  once which stage failed and which were cancelled. `--keep-going` restores the old behaviour.
  A check with job names, and an `--affected` check, never cancel: they are reruns of what the
  person chose. A stage that attached to another call's identical running job (0075) waits for it,
  because that job is not this call's.
- **`test_threads` caps nextest per job.** A host key `test_threads = N` in the hosts file: the
  farm sets `NEXTEST_TEST_THREADS` to the lower of N and the slot's share of the cores. It is the
  environment variable, not `--test-threads` on the command line, because the heavy job is
  `cargo xtask check test`, where the nextest command line is xtask's; the variable reaches it and
  any other nextest, and the command line of a `farm cargo nextest run` can still override it. The
  cap never raises the share. `CARGO_BUILD_JOBS` keeps the share. `farm-b` has 16 cores and two
  slots, so its share is 8 and the failures happened at 8; the owner sets a lower value
  (try 4) in the hosts file and reads the timeouts in `farm status` afterwards. The suggested value is a
  guess to tune, as `max_load = 6` was (0080).
- **The slow timeout is not the farm's.** The 100 ms budgets and the 5 s ceiling of the `ci`
  profile are in `.config/nextest.toml`, which the farm never changes (see The test budgets). The
  farm only picks the profile, and so there is no farm key for it. Raising the `ci` ceiling is a
  change to that file.

**Why:** the cheapest work is the work not started, and the cheapest disk is the copy not kept; the
first four fixes remove waste the farm itself made. **Reversal:** `--keep-going` and `test_threads`
are opt-outs; a seed that proves colder than a fresh copy is removed by deleting the touch and the
`target/` rule in the seed program.
