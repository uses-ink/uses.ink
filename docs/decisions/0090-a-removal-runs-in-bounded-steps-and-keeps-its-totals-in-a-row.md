---
number: "0090"
title: "A removal runs in bounded steps and keeps its totals in a row, so a resume gives a whole receipt"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0090 A removal runs in bounded steps and keeps its totals in a row, so a resume gives a whole receipt

Built (2026-11-28), the native part of deletion with a receipt (package C3). It keeps 0022's order, bytes
before rows, and changes two things: no call deletes an unbounded number of blobs, and a resumed removal
reports all of it.

- **Steps.** `Control::remove_step(site, at)` deletes at most `Policy::removal` blobs (500; `--remove-step`,
  `INK_REMOVE_STEP`) and answers `Removal::More(Progress)` or `Removal::Done(Receipt)`. `remove_site` is
  the loop over it, so the API's `DELETE /v1/sites/{site}` is unchanged. A hosted worker that must stay inside
  a request's budget calls `remove_step` once per invocation; that wiring is not native work and is left.
- **A row of its own.** Migration 304 makes `removal` (site, account, handle, counts of deploys, builds and
  credentials, blobs and bytes deleted). The first step writes it in the same batch that marks the site
  `removing` and drops its live pointer. The counts are taken then, before any row goes, so the receipt is the
  same whichever step finishes it. Each step adds what it deleted to the row, and the last step's batch deletes
  the row with the site's.
- **A failing store.** A step that fails partway records the blobs it had deleted before the failure, then
  returns the failure. Calling again resumes from what is left. A crash between a delete and its record loses
  that step's count (at most one step's share), which makes a receipt too low and never too high; the blobs
  themselves are gone either way.
- **Receipt.** `Receipt::blobs` and `bytes` are the whole removal's. The earlier text, "counts only what was
  left", is replaced, and its test now expects every blob.
- **Not bounded.** `Blob::list` returns every blob of the tenant at each step, so a step is bounded in deletes
  and not in listing; a store whose listing is dear needs a paged `list` on the port, which is a change to
  `blob`. A listing that keeps naming a blob its `delete` reports missing would not end; no adapter does that.

**Reversal:** `remove_site` alone restores the single call; the `removal` table can stay unused.
