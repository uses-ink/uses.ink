---
number: "0172"
title: "The self-host guide is read from the CLI's flags, and the image check runs the command the guide gives"
status: built
date: 2026-11-28
area: release
supersedes: []
---

## 0172 The self-host guide is read from the CLI's flags, and the image check runs the command the guide gives

Built (2026-11-28). `docs/selfhost.md` is published with the docs site and linked from its navigation. It
covers install, a first run, a reverse proxy, the container, the data folder, backup and restore, upgrade
and every limit flag of `ink serve`.

- **Flags come from the source.** Each flag, default and environment variable in the guide is read from
  `crates/surface/cli/src/grammar/args.rs` and the `selfhost` modules, which are what `ink serve --help`
  prints. There is no generator: a flag that changes is a line of the guide that changes, and a reviewer
  who touches the flags is pointed there by the guide's own first paragraph. A generated table is the next
  step if the two drift (the CLI's help is typed data already).
- **The image's command is not the guide's.** The image's default command serves a folder and turns the API
  off, which a stranger would not expect from the word self-host; the guide's compose file therefore names
  `--data`. `cargo xtask release image` starts the image with the guide's flags (`--addr`, `--data`), so the
  command the guide gives is the one that was run.
- **What is stated and what was run.** Run: the image build, its user, its healthcheck request, a read-only
  start with every capability dropped. Written from the code and not run: the compose file as a file, the
  Caddy example, backup and restore of the data folder, and an upgrade across versions (there is one version).
  The guide says which.

**Why:** a guide that is wrong about a flag fails a stranger at the first command. **Reversal:** none.
