---
number: "0112"
title: "The GitLab authority checks the secret token header over digests and proves a repo by a committed challenge"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0112 The GitLab authority checks the secret token header over digests and proves a repo by a committed challenge

Built (2026-11-28) against typed fakes and recorded fixtures only.

- **GitLab does not sign the body.** It sends the secret token itself in `X-Gitlab-Token`. The check is therefore a compare
  of two secrets, done in constant time over the SHA-256 of each side, so neither the first differing byte nor the length of
  our secret leaks. It cannot detect a tampered body: whoever sees one delivery on the wire has the token, so the hook must
  be `https`, and the README of the crate says so. A body is parsed only after the token held.
- **Delivery id.** `X-Gitlab-Event-UUID`, or the `Idempotency-Key` older versions send, goes through the replay window of 0110.
  The event must be `Push Hook`; `Tag Push Hook` and the rest are ignored with the event named.
- **Subgroups.** A project path has up to twenty segments, so `Path` does, and the repository files API takes the path as one
  percent-encoded segment (`octo%2Fnotes%2Fdocs`), which `Host::url` builds.
- **The proof is the repo challenge** of 0111: `GET /api/v4/projects/{path}` for the default branch, then
  `GET /api/v4/projects/{path}/repository/files/.well-known%2Fuses-ink/raw?ref={branch}`, both with no credential, so a
  private project is `repo.proof.missing`. Hosts are an operator's list (`gitlab.com` by default); any other is `repo.host.unlisted`.
  OAuth is not built, for the reasons of 0111.
- **Statuses** are `POST /api/v4/projects/{path}/statuses/{sha}` with `state`, `target_url`, `description` and `name`. GitLab's
  states are `pending`, `running`, `success`, `failed` and `canceled`, so a failed build and a build that could not run are
  both `failed`. The token moves into the call by value.
