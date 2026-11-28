---
number: "0088"
title: "Budgets are one typed Limits value, counted from the rows and the blob list, and a deploy over storage is refused whole"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0088 Budgets are one typed Limits value, counted from the rows and the blob list, and a deploy over storage is refused whole

Built (2026-11-28), with the owner's placeholders of `docs/hosted.md` HQ8 (package C2). It implements 0021
and the operator side of 0044 for the control plane.

- **One value.** `control::Limits` holds `stored` (1 GiB per account), `builds` (60 in 24 hours per account),
  `sites` (10 per account), `deploys` (20 kept per site) and `time` (10 minutes a build). The first, second,
  fourth and fifth are HQ8's; ten sites is this package's placeholder, since HQ8 gives one site per handle and no
  count per account. `Limits::list` is what the public page `/limits` renders and `Limits::over` is what every
  refusal is made with, so the page and the refusals cannot disagree. `Policy::limited` and
  `Control::with_policy` carry it.
- **Flags.** `--limit-stored <BYTES>`, `--limit-builds`, `--limit-sites`, `--limit-deploys` and
  `--limit-build-time <SECONDS>`, each with an `INK_LIMIT_*` variable, each with its default in its help.
  The defaults apply to a self-host too: there is no unlimited setting, an operator sets a large number.
- **Refusals carry `{value, limit}`.** `Refusal::Over { spent, value, limit }` has the codes
  `budget.stored.over` (`size`, `limit` in bytes), `budget.builds.over` and `budget.sites.over` (`count`,
  `limit`). The API answers 413, 429 and 403, with the numbers in the body's `fields`.
- **No counter table.** Migration number 302 is not used. Bytes stored are the sum of `Blob::list` over the
  account's sites, builds are the rows of `job` in the last 24 hours, sites are the rows of `site`, so a count
  cannot drift from what it counts. The cost is a listing per account at each deploy; a hosted store that
  makes that dear adds a counter then, behind the same methods (`Control::stored`, `count_builds`, `count_sites`).
- **A deploy over storage is refused whole (hosted.md Q6).** The blobs of a deploy are in the store before the
  deploy is recorded (`Control::deploy` stats each), so they are counted. Over the limit, the site's older
  deploys are swept first, as they will be once this one is recorded; still over, the blobs only this deploy
  brought are removed, nothing is recorded and the refusal is returned. A build that uploads over the budget
  is caught here, at the deploy, not while it uploads: stopping bytes in flight belongs to the blob adapter.
- **Retention is a sweep, `Control::sweep`.** After each deploy the newest `deploys` deploys, the live one and
  the one it replaced are kept, and a dropped deploy's row goes with the blobs only it named. A blob is removed
  only when a dropped deploy's manifest names it and no kept deploy's does, so nothing a push has uploaded and
  not yet published is touched. Bytes go before rows (0022); a manifest that is not canonical stops the sweep.
- **Build time is a lease, not a kill.** The worker holds a request for at most `time` (`--worker-lease` cannot
  raise it), so a build that runs past it is given to another worker. Stopping a running build is the
  builder's: the container's alarm in hosted, which is not native work, and the `Build` port has no deadline
  today.

**Reversal:** a budget that bites too early is a flag. A counter table is a change under three methods.
