---
number: "0110"
title: "The GitHub authority proves a repo by the App's installation and makes installation tokens one call at a time"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0110 The GitHub authority proves a repo by the App's installation and makes installation tokens one call at a time

Built (2026-11-28) against typed fakes and recorded fixtures only. No GitHub App is registered and nothing here
reaches GitHub; registering the App is the owner's step (`docs/hosted.md` HQ13).

- **Ownership is the installation.** `App::owns` asks `GET /repos/{owner}/{repo}/installation` with the App's JWT.
  The repo is proven when the answer is an installation that is not suspended, whose account is the repository's
  owner and is the account that claimed it. A 404 is `repo.app.missing`, a suspended installation is
  `repo.app.suspended`, another account is `repo.account.mismatch`. The claimed account comes from the user OAuth
  step of `docs/architecture.md` section 12, which is not built here; this takes it as a typed `Segment`. Only an
  owner or admin of an organization can install an App, so the installation is also the admin check.
- **An installation token is made for one use and never stored.** `App` has no token field. `App::token` exchanges
  the JWT for a token scoped to the one repository with `contents: read`, `metadata: read` and `statuses: write`
  (the permissions of HQ13) and returns it as a `Secret`, which prints as `Secret(<redacted>)`, is zeroed when dropped
  and is neither `Clone` nor `Serialize`. `App::status` exchanges a fresh token for each status and lets it drop
  with the request. A reply's `Debug` shows its status and size, never its body (a reply can carry a token), and the
  body is zeroed when dropped. Tests assert that two statuses exchange twice and that no `Debug` of the App, the
  requests or any error contains the token.
- **The JWT is RS256 through `ring`.** `ring` is already the tree's TLS provider and has its `deny.toml` exception, so
  signing adds a direct dependency and no new crate; the `rsa` crate in the lockfile is there only for webauthn and
  its release is new. `Key::read` takes the PEM GitHub issues (PKCS#1, `BEGIN RSA PRIVATE KEY`) or PKCS#8. Claims are `iat`
  60 s in the past, `exp` 9 minutes ahead and `iss` the App id. `ring`'s RSA signing wants a `SecureRandom`, and
  the Worker is a wasm target where `ring` needs a JS feature, so signing sits behind the `Signer` trait: the native
  edge uses `Key`, and the Worker adapter is a WebCrypto `Signer` (hosted work, not built here).
- **Webhook intake is the same order for every forge.** `Hooks::receive` checks the body size before reading it,
  then `X-Hub-Signature-256` (`sha256=` and the hex HMAC-SHA256 of the raw bytes) with `ring::hmac::verify`, which
  is constant time, after decoding the hex and checking its length, so hex text is never compared with `==`. Only then
  does the delivery id go to the replay window, so a stranger cannot fill it. A wrong secret, a tampered body and a
  malformed signature are one refusal, `hook.signature.invalid`, which says nothing about which.
- **The replay window is a bounded memory and says what it is not.** `X-GitHub-Delivery` is not under the signature,
  and GitHub's "redeliver" reuses the id. So the window (default 10 minutes, 10 000 ids, both settings) refuses the
  same id twice and forgets the oldest past its capacity, and a replay with a fresh id passes. What makes that
  harmless is the build queue's dedupe on site and commit (decision 0052): a replayed push for a commit that is built
  or queued queues nothing. The memory is per process; a Worker needs it in a Durable Object (hosted work).
- **A push is typed or ignored with a reason.** `Push { repo, branch, revision }` has a `Repo` (host and lower-case
  path), a `build::Branch` and a commit as a `build::Revision` of 40 or 64 lower-case hex digits. A verified delivery
  that queues nothing is `Received::Ignored` with a typed `Ignored`: not a push (`ping`), a repository no site follows,
  a tag or other reference, a branch name the build does not read, or a deleted branch (a commit of zeros).
- **What is shared is one crate.** GitHub and Forgejo use the same HMAC scheme and all three forges need the same
  names, `Push`, replay window, `Secret`, `Transport` and `Fake`, so `uses-ink-authority-forge` holds them and each
  forge crate holds only the forge's spelling. It depends on `uses-ink-build` alone and knows no control plane.
- **Commit statuses** are `POST /repos/{owner}/{repo}/statuses/{sha}` with `state`, `target_url`, `description` (cut to 140
  characters by `Status::note`) and `context` (`uses.ink` by default).
- **Left.** Registering the App and storing its key and webhook secret through `secrets_request` (owner), the user OAuth
  step that gives the claimed account, a real `Transport` adapter over `reqwest` or the Worker's fetch, the WebCrypto
  `Signer`, and the wiring of decision 0113.
