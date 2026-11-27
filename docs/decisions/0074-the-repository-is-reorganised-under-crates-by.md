---
number: "0074"
title: "The repository is reorganised under `crates/` by layer; monoliths are split into directories first"
status: accepted
date: 2026-11-27
area: repo
supersedes: []
---

## 0074 The repository is reorganised under `crates/` by layer; monoliths are split into directories first

Accepted, owner (2026-11-27), with every open choice of `docs/layout.md` section 15 taken as
recommended. `docs/layout.md` holds the proposal, the move map, the work packages and the risks.
In short:

- Every workspace crate moves under `crates/<layer>/` (`core`, `port`, `server`, `runtime`,
  `surface`, `standalone`), families whole (`crates/port/blob/dir`). `xtask` moves to `tools/`.
  Crate names do not change; the `Names` rule reads the path below the layer. `trace` is renamed
  `logging` and `queue/local` and `build/local` get mechanism names.
- `cli`, `control`, `model`, `farm`, `xtask` and `edge` are split into directories first. Only
  `control` gains crates (`rows`, `signin`, `keys`), because its subsystems share its rows.
- Duplicated facts get one home: the output-folder layout in `model`, the API views in `edge/api`,
  the diagnostic catalog in `diag`, the WIT mirror list in `wit/mirror`.
- `decisions.md` becomes one file per decision with a generated index, and the `docs` stage gains
  link, citation, crate-path, front-matter and index checks, and a directory fan-out check that
  only reports. The `area` of the front matter is a free label, a non-empty lowercase slug, not a
  list in `.config/layout.toml`: areas follow the work, a closed list is one more row every new
  decision would have to merge, and the generated index shows a mistyped label next to the right
  one.
- Every work package is a jj commit that passes `farm check`; the physical move is one commit in
  a freeze, after the other work in flight has landed.

**Why:** the root has 50 entries against a rule of about eight, `cli` and `control` hold several
jobs each, and several facts are written down twice.
**Reversal:** none of the packages before the move depends on it; the move itself is reverted by
the inverse commit.
