---
number: "0111"
title: "The Forgejo authority proves a repo by a committed challenge, lists the instances it calls, and ships Codeberg"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0111 The Forgejo authority proves a repo by a committed challenge, lists the instances it calls, and ships Codeberg

Built (2026-11-28) against typed fakes and recorded fixtures only. No OAuth application is registered anywhere.

- **The proof is the repo challenge, not OAuth.** `docs/architecture.md` section 12 names OAuth per instance with the repo
  challenge as the baseline. OAuth needs an application registered on each instance (outward-facing, and an
  instance admin's call) and a redirect flow with state, none of which can be tested offline. The challenge needs
  neither: the person commits a line `uses-ink-repo=<32 hex digits>` to `.well-known/uses-ink` on the default branch of a
  public repository, and `Forgejo::prove` reads it from `GET /api/v1/repos/{owner}/{repo}/raw/.well-known/uses-ink` with no
  credential. Write access to the repository is what it proves, and the owner namespace is the first path segment.
  OAuth stays the planned upgrade for private repositories and for proving the account without a commit; it is not
  built. The same line and file serve GitLab (0112), and the token is compared in constant time through `Secret`.
- **The challenge is drawn by the caller** (16 bytes, like the DNS token of 0093) and is not stored by this crate; binding a
  proven repo to a site is the control plane's, as 0093 does for a domain (decision 0113 describes it).
- **Only listed instances are called.** A host a user types would otherwise make the server send requests where they
  say, including to internal addresses. `Instances` is a list of hosts an operator chose; `prove` on any other host is
  `repo.host.unlisted` and sends nothing. The list is `crates/server/authority/forgejo/instances.toml`, read once by `Instances::bundled`
  with a typed `Unreadable` (exactly one primary, no host twice, every host a plain DNS name), and an operator reads
  their own with `Instances::read`.
- **Codeberg is the pre-registered default.** The file has `codeberg.org` as the primary instance with `preregistered = true`.
  The flag records that the project's OAuth application is meant to be registered there; the client id is a deployment
  secret and is not in the file, and no application was registered.
- **Webhooks are Gitea's scheme.** Forgejo signs the body with the hex HMAC-SHA256 under the webhook secret, with no
  prefix, in `X-Forgejo-Signature` (Gitea's `X-Gitea-Signature` is read when the Forgejo one is absent), and sends
  `X-Forgejo-Event` and `X-Forgejo-Delivery` the same way. The check, the replay window, the typed `Push` and the ignored
  reasons are the ones of 0110.
- **Statuses** are `POST /api/v1/repos/{owner}/{repo}/statuses/{sha}` with the token of the instance's application or a bot.
  The token moves into the call by value and is dropped with the request.
