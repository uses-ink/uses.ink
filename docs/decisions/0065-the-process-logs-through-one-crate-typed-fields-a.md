---
number: "0065"
title: "The process logs through one crate: typed fields, a span per layer of work, a channel of its own"
status: accepted
date: 2026-11-26
area: logging
supersedes: []
---

## 0065 The process logs through one crate: typed fields, a span per layer of work, a channel of its own

- **One crate.** `logging` owns the subscriber and the span macros; no other crate sets up logging.
  `tracing` is the facade, `tracing-subscriber` supplies the registry and `Targets`, with no `fmt`,
  `env-filter` or regex. The human tree, JSON lines and timing table are one layer in `logging`,
  compiled only for the CLI (`layers`).
- **Typed fields.** `tracing::Value` is sealed, so `logging::Field` stands in for it: call sites name
  `Size`, `Count`, `Took`, `Digest`, `Secret`, `Failure` and enums with `Kind`, never a format
  string. Event and span names are constants. An error is logged with its whole `source()` chain
  as structure. Conversions are macro arguments, so a disabled event runs none.
- **Spans.** command, build, stage (fetch, discover, source, decide, image, render, write), unit,
  plugin call, request, api, job, statement, blob. Async code uses `.instrument`.
- **Channel.** Off by default; never stdout, never inside `--json` or among its events. `-v` count,
  `INK_LOG`, `--log`; `INK_LOG_FILE`/`--log-file`, `INK_LOG_FORMAT`, `INK_LOG_SPANS`,
  `INK_LOG_TIMINGS`. `RUST_LOG` is not read. Under `--json` the log goes to a file or nowhere.
- **Redaction.** `Secret` prints nothing; no body, header or statement text is logged; a request
  path is logged without its query.
- **Declined.** `tracing-error` (errors are typed and logged inside their spans) and
  `#[instrument]` (it records through `Debug`). **Tests.** `logging::Capture` asserts on stage
  order, an edge refusal and a failed job.

Supersedes the `INK_LOG`/`RUST_LOG` sentence of `conventions.md` and `cli.md` section 3; keeps
`-v` as it was for output. **Why:** after 0061 the output is exact but nothing says why a build
took a minute, why a page re-rendered or why a request was refused. **Reversal:** if the span
layer costs more than it explains, drop the layers and keep the typed fields.

**Instrumented since (2026-11-27).** Beyond the build, the engine, the hosts, the edge, the API,
the control plane, the database and the blob store: the edge's lookup over the control plane
(`sites.serving`, `sites.asked`, `sites.placed`, `sites.failed`, `sites.stopped`); the folder
adapter (`folder.opened`, `folder.loaded`, `folder.refused`); the watch's git head read
(`git.head`, a `debug` detail, with a `warn` only for a refusal by the network setting); the
supervisor of the watch and the worker (`task.started`, `task.crashed`, `task.restarting`,
`task.gave_up`, `task.stopped`); the local queue (`queue.sent`, `queue.received`,
`queue.settled`, a `warn` when a lease had already ended); the local secret adapter (`key.opened`,
`key.refused`, `secret.sealed`, `secret.opened`, by outcome only) and the disk cache's sweep
(`cache.stayed`, one `warn` per sweep with a count). A lookup served from memory, an empty queue
poll and a cache hit write nothing.
