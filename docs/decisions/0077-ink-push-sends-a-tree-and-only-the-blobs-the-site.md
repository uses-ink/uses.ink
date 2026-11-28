---
number: "0077"
title: "`ink push` sends a tree and only the blobs the site lacks; the folder verbs know their site by `.uses/site`"
status: accepted
date: 2026-11-27
area: cli
supersedes: []
---

## 0077 `ink push` sends a tree and only the blobs the site lacks; the folder verbs know their site by `.uses/site`

Accepted (2026-11-27). Builds on 0046 (source plugins), 0052 (the API and its intake), 0061 (the
command tree) and 0068 (signed requests). `ink init`, `push`, `status`, `watch` and `export` were
the verbs that exited 2 as pending; they are built.

- **Content-addressed upload, no session.** The CLI reads the folder into a `uses.tree/v1` document
  (`build::Tree`: path, blob hash, size, sorted, canonical JSON) and posts it to
  `POST /v1/sites/{site}/source`. The server answers `missing` (the hashes the tenant's blob store
  lacks, each once, with the byte limit of one request) or `queued` (the tree is stored, a build is
  queued). The CLI sends the missing blobs in tar archives named by hash with
  `PUT /v1/sites/{site}/source` and posts the tree again, up to `--rounds` times. The server keeps
  nothing between requests: the tenant's blobs are the state. **Resume is running `ink push`
  again.** It sends only what is missing, and says how many files were sent and how many were held.
  Each blob is checked against its name on the server.
- **Why tar batches.** The nonce store of 0068 holds 1024 nonces per key inside the skew window, so
  a push of thousands of one-file requests would be refused. A batch is also the unit of memory.
- **A tree rides on the job.** A new `bundle` table maps a build job to its tree hash (migration
  `Treed`, number 3; the `db` port has no ALTER, so a table, not a column). Two pushes in a row
  cannot build each other's tree. The source of a site stays `Source::Upload`; a build request
  carries `Source::Bundle(hash)`, and `crates/runtime/build/process` reads the tree and its blobs from the tenant
  into a snapshot of version `Upload`. `GET /v1/sites/{site}/source` returns the latest tree's
  files as a tar archive, which is what `ink export --source` downloads.
- **Bounded memory is the streaming answer.** Intake is not streamed. Every request is read whole,
  inside two settings of `Limits`: `tree` (8 MiB, `--upload-tree`) and `part` (64 MiB of files in
  one request, `--upload-part`), beside the existing file, count and export limits. The CLI cuts
  its batches by `--batch` (bytes) and `--batch-files`, whichever is smaller than the server's.
- **No wasm `ink:source-upload` component.** Under 0046 a source plugin is a fetch rule that runs
  over host-provided bytes (a git remote, a registry). An upload has nothing to fetch: its bytes are
  already in the tenant's store. The native `Source::Bundle` arm is the whole adapter.
- **The link file.** `ink init` writes `.uses/site` (TOML, `schema = "uses.link/v1"`, `api`, `site`,
  `handle`), atomically. It holds no credential: the key is `ink login`'s, in the config folder.
  `.uses` is a version-control folder for the folder reader, so it is never content. `init --handle`
  makes an upload site, `init --site` links one that exists; a different link is refused without
  `--force`.
- **Which site a verb means.** The folder verbs (`push`, `status`, `watch`, `export`) take the site
  from `--site`/`INK_SITE`, else the link file, and never from `account.site` of the config file, so
  a folder that is linked to nothing cannot publish to the site the config names. No link is exit 2
  with the fix `ink init`. The address comes from the flag, the link, then the config. `deploy`
  verbs keep the old rule.
- **Retries.** Reads and the idempotent writes (the tree post, a blob batch) are tried again on a
  connect failure, a timeout or a 502 to 504, `--retries` times (default 2), waiting `--retry-wait`
  milliseconds, doubling. Nothing else is retried.
- **Exit codes of `push`.** 0 when the deploy is live; 1 when the build was refused (the first
  problem of its log is the cause); 2 for no link; 3 for auth; 4 for network, a failed build or a
  wait that ran out (`--wait` seconds, the build keeps going); 5 when the deploy is recorded and not
  live because publishing is paused after a rollback, with the fix `ink deploy resume`. A failure
  during upload says how many files the site holds (`state: uploaded`), and exits as its cause does.
  Stdout is the bare deploy id (the build id with `--no-wait`); `--json` is `uses.push/v1`.
- **`watch` follows state, not stages.** The server stores a build's log when the build ends, so
  `ink watch [BUILD]` polls the builds (`--poll` milliseconds, `--wait` seconds) and says queued,
  running, ended; the live stage meter of `ink build` has no source on a remote build yet. A TTY
  gets the meter line, a pipe gets plain lines. With no argument it follows the running build, or
  waits for the next one.
- **`export` is atomic.** The archive is read in memory within `--max-files`, `--max-file`,
  `--max-bytes` and `--max-name`, written into a staging folder beside the target and renamed. A
  failure removes only the staging folder. A target that holds anything is refused. The live site's
  files are named by route (`index` for the root), not by the folder `ink build` writes.
- **`status`** is the API's overview plus a word for health (`ok`, `building`, `paused`, `failing`,
  `empty`). The API gained `pending` builds in it. Domains are not exposed before M4, and the output
  says so.

**Why:** a push that sends the whole folder each time wastes the network, and a protocol that needs
server-side upload state needs cleanup. **Reversal:** streaming intake would replace the whole-body
limits; a hosted store with presigned URLs would replace the tar batches.
