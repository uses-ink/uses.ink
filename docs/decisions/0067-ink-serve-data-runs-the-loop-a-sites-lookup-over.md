---
number: "0067"
title: "`ink serve --data` runs the loop: a sites lookup over the control plane, the watch and the worker as supervised threads, git through the source plugin"
status: accepted
date: 2026-11-26
area: edge
supersedes: []
---

## 0067 `ink serve --data` runs the loop: a sites lookup over the control plane, the watch and the worker as supervised threads, git through the source plugin

Accepted. 0052 left four gaps between a built control plane and a self-host that runs it. This
closes them.

- **Sites.** `edge/control` is the edge's `Sites` over `Control`: host to site (`Origin::handle`),
  site to live deploy (`Control::serving`), deploy to decoded manifest (`Control::manifest`), and
  the blobs read from the tenant's folder of the blob directory through `edge_folder::Files`, so
  link refusal, swap safety and range reads are the ones the folder server has. The edge's
  `Sites` is `Send` and the control plane's futures are not, so the lookup is a channel as the
  API's `Bridge` is: `Back::pair` gives the `Fronts` the server calls and the `Back` that runs
  on the caller's task. The answer for a host is remembered for `--site-ttl` seconds (a deploy or
  a rollback shows within that), decoded deploys are kept by `(site, deploy)` up to
  `--site-deploys`. `Fallback` asks the control plane first and the folder second, so a host that
  is no site still gets the folder.
- **Media types need no migration.** The brief expected a typed field. The stored manifest's
  canonical bytes already hold each route's media essence, status and headers; the control plane
  discarded them when it listed a deploy. `Manifest::from_canonical` (in `model`, with its typed
  `Canonical` error and a re-encode check, so bytes that are not canonical are refused) reads them
  back, and `Listed` keeps the manifest. Manifest v3 has no redirects or variants, so none are
  served and `Deploy` keeps its empty `moved` and `variants`.
- **Last seen.** The watch keeps the last commit it queued per site in the `seen` table
  (`Remembered`, migration number 2, a typed table with a foreign key to the site, wiped with the
  site). A restart reads it and queues nothing for a source that did not move. The queue's dedupe
  key (site and commit) still covers a lost row. A new site is first polled within the
  `--poll-lookout` (default 5 s, floor 1 s), not after the poll interval, which has a floor of
  10 s.
- **Shared ports.** `Arc<P>` is `Ports` when `P` is, and `Arc<C>` is a `Clock`, so the API, the
  lookup and the loops are `Control`s over one set of adapters. SQLite is a mutex over one
  connection in WAL mode, the blob directory writes by atomic rename.
- **Loops.** `ink serve --data` runs the watch and the worker, each on its own thread with its
  own current-thread runtime, under a supervisor (`cli/src/selfhost/tasks.rs`). A panic or an early
  return is a typed event; the loop is started again after `--restart-delay`, and left stopped
  after `--restart-limit` crashes in a row (a run longer than `--restart-healthy` forgets them).
  A stop (Ctrl-C) is graceful: a loop finishes its step, and its next wait returns as stopped.
  Every event (started, queued, duplicate, unreachable, built, refused, dropped, failed, crashed,
  restarting, gave up, stopped) is one line on stderr with its time, and a test waits on them.
- **Git heads.** `watch/git` is `Heads` over `gix`: it connects as `git ls-remote` does, reads the
  advertised references, picks the branch, tag (peeled) or commit the site names, and fetches
  nothing. `Heads::head` now takes the site's `Access`, which `Access::of` opens from the site's
  `git-token` credential: `user:password`, or a token alone sent as the user `git`. It redacts
  itself, is zeroed on drop and appears in no event.
- **Git builds.** `build/process` takes a `Remote` for a git source (`Offline` by default, which
  keeps `Unsupported`). `ink` implements it in `cli/src/command/remote.rs` over the bundled
  `ink:source-git` component (0046): a `Fetch` with the operator's network, cache and a time
  budget (`--git-time`), the site's credential held under one grant name for the call. Every
  `Unsourced` becomes the diagnostic of a refused build; only a missing component, an unloadable
  one or an unreadable credential is a failure the queue retries. A build that was refused for a
  transport that failed is not retried until the source moves or a build is asked for.
- **Which git.** `--git-scheme` (repeatable, default `https`) is the one list the API checks
  `POST /v1/sites` against and the fetch's `Network` is derived from (`file` alone is `Local`).
  The API and the build now use the model's `Locator`, so an address that is neither `file` nor
  `https` is a body the API refuses.

**Not done:**
- **HTTPS fetches (done in 0072).** `gix`'s HTTP transport was not enabled: it needs `curl` or `reqwest` with a
  TLS stack (`rustls`, `ring`), none of which is in the tree, and their licences are not on
  `deny.toml`'s allow list. `host_native::Fetch` and `Remotes` therefore reach `file://` only and
  say `Refused::Transport` and `Unlisted::Transport` for `https`. A self-host over https needs
  that decision first. SSH is not a scheme of the model's `Locator`.
- **Private repositories over `file`:** the credential path is built and tested to the grant, but
  the `file` transport has nothing to authenticate, so it runs for https only.
- **Streaming.** Intake and export hold the whole body in memory: `edge_axum::Request` and
  `Payload` are shared with the sign-in routes, and a streaming body changes both. The limits
  (`--upload-bytes`, `--export-bytes`) bound it. Not changed here.
- `ink push`, redirect and variant entries.

**Why:** the loop is the product's first end to end path, so every step is a typed value the
next one reads, and nothing between them waits on a sleep. **Reversal:** if a thread per loop is
too much for a small box, run both on one runtime with `spawn_local`; the supervisor's events and
the settings do not change.
