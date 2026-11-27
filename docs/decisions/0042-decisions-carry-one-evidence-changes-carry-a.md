---
number: "0042"
title: "Decisions carry one evidence; changes carry a cause; inference rules are versioned"
status: accepted
date: 2026-11-26
area: engine
supersedes: []
---

## 0042 Decisions carry one evidence; changes carry a cause; inference rules are versioned

Accepted, owner (`research/inference.md`). A decision is its value and the evidence that chose
it; rejected alternatives leave the model, and `ink explain --why <path> <code>` replays one
ladder on request, rung by rung, including the rungs that found nothing. Every build is compared
with a baseline (the live deploy hosted; locally the previous `ink build`, whose output folder
keeps a decision record beside the manifest so causes can be told apart) and reports a change
set in which each change has a typed cause: content, config, rules, plugin, ability or engine.
Newly public routes head every change set, and a private source refuses any publication that no
stated selection made. Inference rules have a version, decided as `site.rules`: a pin, else the
baseline's, else the latest. A newer ink never changes a site's decisions on its own: what its
rules would change is shown as pending until `ink update` or a dashboard action accepts it.
`ink explain --pin` is removed; `ink pin <path> <code>` writes one pin. The owner's answers:
a deploy may hold the rules version when the source states none, labelled as held outside the
source (E1, refines 0018); old rules are kept for two rule versions or twelve months, whichever
is longer (E2); bundled theme updates go live without acceptance and are listed in the change
set (E3); the first hosted deploy suggests committing `rules = n` as a notice and never writes
it (E4); a change attributed to the engine is a warning locally and holds a hosted deploy (E5).
Built in two steps: one evidence, `--why`, the change set, the newly public guard and `ink pin`
in M2; versioned rules and amendments before hosting, in M4. **Why:** the jobs that cannot be
undone (a page made public, a site moved by our upgrade) are questions about change between
builds, which per-value evidence cannot answer; locking rules instead of values keeps content
free and needs no commit, so vaults and uploads are covered; the baseline is state already kept
for rollback. **Reversal:** amendments are needed more than about once a quarter, or replaying
interpret under two rules costs more than 10% of build time on the largest fixture: drop pending
replays and keep a published rules changelog with the change set alone. Refines 0011 (bounds
stay facts), 0018 and the M1 exit criteria ("pin reproduces the deploy" is dropped: it tested
that pins freeze content).

As built (M2, step 1): `Decision<C>` is subject, value and evidence; `Evidence::basis()` says
`Stated`, `Inferred` or `Assumed`. `ink explain --why [path] <code>` replays one ladder through
`Engine::trace` (`uses.trace/v1`); `uses.explain/v2` drops `rejected`. `ink pin [path] <code>`
writes one pin (`uses.pin/v2`, `--dry-run`). `ink build` writes `uses.manifest/v3`, whose
`record` holds the build's inputs, decisions, files and routes, and prints the change set
against the previous record on stderr (in `uses.build/v2` with `--json`); a record it cannot read
is a notice, never a refusal. The cause order adds two steps to `research/inference.md` 7.3: a
changed `ink.toml` is `config`, and a change to any other file is `content` before `engine`, since
a file's decision can follow another file (an image a page stops using); and `ability` compares
what the source lacked as well as what it had, so a deepened shallow clone is the source's change,
not ink's. The guard is
`page.publish.unstated`, as diagnostic codes have three words. Bundled plugins are not listed in
the record: they move with ink's release, so `plugin` is given only for components. `rules` is
typed and waits for `site.rules`.

`site.format` (0043) is the first code decided before the engine runs: it chooses the format the
engine is built over, so `engine::Chosen` decides it from the snapshot and the pins and
`Understood::with` adds it. `Evidence::Inline { path, span }` is the evidence of a value found
in a page's text; it is `Basis::Inferred`.
