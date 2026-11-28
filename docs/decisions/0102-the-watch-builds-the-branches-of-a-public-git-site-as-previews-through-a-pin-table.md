---
number: "0102"
title: "The watch builds the branches of a public git site as previews, through a pin table beside the job"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0102 The watch builds the branches of a public git site as previews, through a pin table beside the job

Built (2026-11-28), the reach of decision 0091: until now a preview was pushed by archive. The watch now finds the
branches itself, and `ink preview list|remove` read and end them.

- **A side table, not a column.** The `job` table gets no column. Migration 308 adds `pin` (job id, site, branch,
  commit), the way `bundle` carries a pushed tree, so the migration only adds and runs the same on SQLite and D1.
  `Job.pin` and `build::Request.pin` say which branch a build is for.
- **The control plane picks the source.** `Control::request_branch` sets the request's git source to the branch,
  so a builder reads `source` and needs no change. A fetch asks the remote for refs (a commit id is no ref a clone
  can be asked for), so a build that runs later builds the branch as it is by then. The commit the watch saw is
  what it dedupes by: a branch that moved is a new pin and is built again, under the same label.
- **A preview, never a deploy.** A built pin job is settled with `Control::preview` under the label
  `Slug::branch(branch)` (`feature/login` is `feature-login`), and the worker reports `Worked::Previewed`. Preview
  jobs are left out of the dashboard's build history. `BuildView.branch` names the branch in the API.
- **Only public sources.** The watch lists branches only for a site with no credential, since a preview is served
  at an address anyone can open. A source that is not git, or a branch whose name makes no label, is a typed refusal
  (`Refusal::NoPreview`, `Error::Label`) said in the watch's log, not skipped.
- **Capped and observable.** `--preview-branches N` (`INK_PREVIEW_BRANCHES`, default 10, `0` turns it off) bounds
  the branches built for one site by name, and the watch says how many it left out (`watch: site S has N more
  branches than the previews limit`). The branch the site itself follows is not previewed. A restart builds nothing
  again: `Control::pinned` reads the table before a job is queued, and the queue's dedupe key covers the time
  between.
- **Listing the branches fetches no object.** `Heads::branches` reads the same advertised reference list as
  `Heads::head`, under the same network setting, and names the references that are no branch.
- **Removal.** `DELETE /v1/sites/{site}/previews/{label}` (right `Deploy`) ends a preview now with the blobs only it
  held; a label that is not there is `preview.label.unknown` (404), so a typo is not a success. `ink preview list`
  and `ink preview remove <LABEL>` call it. Bare `ink preview` still serves the folder locally.

Left: a webhook that queues a branch at once (the poll finds it within its interval), and previews of private
sources behind a sign-in.
