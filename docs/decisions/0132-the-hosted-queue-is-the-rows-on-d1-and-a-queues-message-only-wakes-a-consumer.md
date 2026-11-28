---
number: "0132"
title: "The hosted queue is the rows on D1, and a Queues message only wakes a consumer that then reads the rows"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0132 The hosted queue is the rows on D1, and a Queues message only wakes a consumer that then reads the rows

Built (2026-11-28), A2 of `docs/hosted.md`; it carries out 0079's first choice.

- **`crates/port/queue/cf`** (`uses-ink-queue-cf`) is `Cf<D, W>`: the rows of `queue-rows` over any `Db` (D1 hosted)
  and a `W: Wake`. `receive`, `ack` and `release` are the rows'; the rows are the truth. The crate builds natively,
  so its tests run over SQLite; only `Queues`, the wake-up through a Queues binding, is `wasm32`.
- **When it wakes.** After a `send` that queued a message (a `Duplicate` is already in the rows, and its own send
  nudged), and after a `release` that took effect, with the release's delay. Not after a `receive`, an `ack` or a
  stale `release`. The nudge's delay is rounded up to whole seconds and held to Cloudflare's 12 hours; a message
  visible later is nudged at 12 hours and found not yet visible, and the Cron trigger covers the rest.
- **A wake-up is never the answer.** A refused nudge is a `queue.woken` warning (new event, `docs/tracing.md`) with
  the cause as a field, and `send` and `release` answer what the rows answered. This is the point of "wake-up only":
  a lost, late or doubled message changes when a message is taken, not whether it is. `Silent` sends none, which is
  the Cron-only default the plan allows (A2 shrinks to a configuration entry for a deployment that takes it).
- **The message** is `{"wake":"queue"}` with the nudge's delay as the message's `delaySeconds`; a consumer reads
  the rows, not the body.
- **The suite.** `queue-conformance` got `Case` (7 cases) like the row and blob suites, `suite!` unchanged. The
  adapter's own cases are `Nudged` (one nudge for a new message, none for a duplicate, one at the delay for a
  release, none for a receive, an ack or a stale release) and `Refused` (every wake-up refused, answers unchanged,
  the message still delivered). They are public in the crate so the Worker rig runs the same ones.

Numbers: native, 10 tests of the crate (7 port cases, the 2 above, 1 unit) pass over SQLite. In workerd, the rig
with the Queues producer binding passes 9 of 9 (36 to 98 ms a case); the rig is 880,733 B raw and 263,691 B gzip
with the db, blob and queue adapters.

**Not done:** delivery timing of a real Queue and the consumer's batch handler (`queue` event), which the Worker
crate of W1 owns; real Queues' 12-hour limit is Cloudflare's documentation, not measured here.
**Reversal:** a consumer needs the Queues payload as the work (not the rows): then the dedupe and the lease move
into a Durable Object, as 0079's reversal says.
