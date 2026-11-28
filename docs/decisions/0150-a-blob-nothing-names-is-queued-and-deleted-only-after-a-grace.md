---
number: "0150"
title: "A blob nothing names is queued and deleted only after a grace"
status: accepted
date: 2026-11-28
area: control
supersedes: []
---

## 0150 A blob nothing names is queued and deleted only after a grace

Accepted (2026-11-28). A review found a race in the sweeps of 0088 and 0091. A build uploads its files, and the R2
adapter skips the upload of a blob the store already holds (0131), so no row of the control plane says that build
uses it. The build then records its deploy, a moment later. If a sweep, the end of a preview or a refused deploy
removes the last row that named that blob in between, the blob is deleted and the deploy that follows is refused with
`MissingBlob`, or, before this decision, could have been recorded over a missing file.

- **A blob is queued, not deleted.** `reclaim` (preview end), `drop_deploys` (the sweep) and `discard` (a refused
  deploy) put the blobs they would have deleted in `doomed` (migration 309): site, blob, bytes and the time it was
  queued. A blob queued again takes the newer time.
- **`reap_site` deletes what is due.** A queued blob is deleted when its time plus the grace has passed and no deploy
  and no preview of the site names it, read again at that moment. A blob named again is taken off the queue and stays.
  `Control::reap` does this for every site and runs at the end of `Control::expire`, so a site that does nothing more
  is still cleaned. Bytes are deleted before the queue rows (0022).
- **Recording a deploy or a preview takes its blobs off the queue** in the same batch, so a build that was slow still
  wins over a sweep that ran before it.
- **The grace is `--blob-grace SECONDS`** (`INK_BLOB_GRACE`), 900 by default: longer than the default build time of 600
  seconds, which is the longest an upload can sit before its deploy. `0` deletes at once and restores the old
  behaviour; a deploy that lost a blob that way is refused with `deploy.blob.missing`, never recorded broken.
- **Queued blobs are not charged.** `Control::stored` subtracts the bytes of a site's queue, so a sweep that frees room
  for a deploy still frees it for the budget of 0088. The store holds the bytes a little longer than the account
  pays for.
- **Site removal is unchanged**: it deletes every blob of the tenant at once (0022), and its rows go with the queue.
- **`sweep` and `discard` take the time**, because the queue is written with it.

**Left, by design.** The window is not closed, it is moved. A build whose upload and deploy are further apart than the
grace can still lose a blob that was queued before the upload; the refusal names it and a retry works. The reaper's own
read of what is held and its delete are two steps, so a deploy recorded between them is still exposed for the length
of one request. A cross-store transaction would close both, and no port offers one.
