# uses-ink-queue-cf

The queue port (`uses-ink-queue`) hosted on Cloudflare (decision 0132): the rows of `uses-ink-queue-rows`
over D1 are the queue, and a Cloudflare Queues message only wakes a consumer.

Cloudflare Queues has no dedupe key, and its consumer is a push batch handler with no lease a Worker
can set, so the port's `Duplicate` answer and its lease have to live in the rows. That is why a
wake-up is all the Queues binding does (decision 0079).

- `Cf<D, W>` is the adapter: `send`, `receive`, `ack` and `release` are the rows', and after a
  `send` that queued a message, and after a `release` that took effect, it asks `W: Wake` for a
  nudge. A duplicate, a receive, an ack and a stale release send none.
- A nudge that fails is a `queue.woken` warning and never the answer of the call: the message is in
  the rows, and a Cron trigger runs the consumer too.
- `Queues` (wasm32 only) is the wake-up through a Queues binding; its message is `{"wake":"queue"}`
  with the nudge's delay as the message's delay, held to Cloudflare's 12 hours. `Silent` sends none,
  for a deployment that wakes on Cron alone. `Recorder` is the fake the cases use.
- `Nudged` and `Refused` are the cases of the wake-up. The port's own suite runs on `Cf` natively
  (over SQLite) and in workerd (over D1) through `cargo xtask worker queue`.

The crate builds natively: only the Queues binding is `wasm32`.
