---
number: "0136"
title: "The Worker reads a repository's heads from the smart-HTTP reference advertisement with no git library"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0136 The Worker reads a repository's heads from the smart-HTTP reference advertisement with no git library

Built (2026-11-28), A6 of `docs/hosted.md`.

- **`crates/server/watch/worker`** is the watch's `Heads` over a `Fetch` port. `watch/git` links `gix` and spawns
  blocking tasks, which a Worker has neither the sockets nor the threads for; the poll needs only one GET and a list of
  packets. `Smart` asks `<repository>/info/refs?service=git-upload-pack` with no `Git-Protocol` header, so every
  forge answers with the version 0 advertisement, which already holds `HEAD`, every branch and tag, the peeled tag lines
  and `symref=HEAD:<branch>`. Protocol version 2 would need a second request (`ls-refs`) per poll for no extra
  information, so a version 2 answer is refused as `Misread::Version`.
- **The reading rules are `watch/git`'s**: no revision is `HEAD`'s commit; a revision is a branch, then a tag (peeled to
  its commit), then a full reference name, then a commit id that names itself; branches come in name order with the
  default marked, and a reference under `refs/heads/` that is no branch to this is kept in `skipped`.
  `Absent::{Unborn, NoHead, Revision}` are `Missing`'s three cases.
- **Fixtures are recorded, not written**: `info/refs` from GitHub (`octocat/Hello-World`), Forgejo
  (`codeberg.org/forgejo/forgejo-docs`) and GitLab (`gitlab-org/gitlab-test`, with branch names such as `'test'` and
  `100%branch`, and peeled tags). Only whole packets of pull and merge request references are dropped, to keep them at
  1 to 11 KB; `fixtures/README.md` says which. The `Case` suite reads each, expecting the commits found by reading the text
  and not by the parser, and the same suite runs in workerd.
- **"The same as `watch/git`"** is a test that makes a repository with `git`, an annotated and a lightweight tag and two
  branches, and reads it through `Remotes` over `file://` and through `Smart` over the output of
  `git upload-pack --advertise-refs`: heads and branches are equal values. It needs `git` on the host.
- **Failures are typed** (`Unlisted<E>`: network setting, scheme, the fetch's error, status, limit, misread
  advertisement, absent revision, bad commit id) and a limit keeps a forge from sending the Worker an unbounded body.
  A login is `Authorization: Basic` and never prints.

Numbers: native 4 tests (the suite's 8 cases, the local comparison, a name check, the encoder) plus 2 unit tests pass; the rig
is 1,185,016 B raw and 370,145 B gzip with every adapter, and passes the 9 watch cases in workerd, one of which
fetches an advertisement through the platform's `fetch` from the rig's own route.
**Not done:** a poll of a real forge from a deployed Worker (private repositories and rate limits are untried), conditional
requests (`If-None-Match`) to make an unchanged poll cheaper, and wiring `Smart<Fetching>` into a product Worker.
**Reversal:** if a forge stops serving version 0, `Smart` adds the `Git-Protocol: version=2` request and an `ls-refs` POST;
`Advertisement` is the only part that reads packets.
