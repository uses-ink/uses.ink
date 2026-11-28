---
number: "0160"
title: "A repository claim binds a proved repo to one site and seals its webhook secret"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0160 A repository claim binds a proved repo to one site and seals its webhook secret

Built (2026-11-28). This is the first piece of the plan of 0113: the `repo_claim` table and the cases over it. Decisions
0161 and 0162 are the route and the adapter.

**One claim, one site.** Migration 311 adds `repo_claim` (repository, site, challenge, secret credential, created, proven).
The key is the repository and the site. A nullable column `owned`, which is `1` once a claim is proven and null before,
carries a `UNIQUE (repo, owned)` index, so the database holds one proof of a repository whatever order sites prove in; two
unproven claims of one repository can exist side by side, as two sites can ask for one name before either proves it (0093).
The repository is the lower-case host and path, written once by `Repo::at`, which strips a `.git` and a trailing `/` and
refuses a port or a user.

**What a claim is for.** A site has one source, so a claim is accepted only for the site's own git source: the address
of the source, as `Repo::at` reads it, must equal the repository. A stranger cannot claim a repository by naming it.
`claim_repo` seals a webhook secret as a `WebhookSecret` credential (0075, the same table and the same sealing as a source
token) and writes the claim, the credential and the event in one batch; asking again replaces the challenge and the secret.
`bind_repo` takes the typed `Proven` the authority returned, sets `proven` and `owned`, and drops every other site's claim of
the repository with its secret in the same batch. It is idempotent. `rotate_repo` puts a new credential in place of the old
one and deletes the old one. `release_repo` deletes the claim and the credential and is allowed for a site that is being
removed. Removing a site wipes its claims (0090). Four events record it: `repo.claimed`, `repo.proven`, `repo.rotated` and
`repo.released`. Three catalog codes are new: `repo.claim.taken`, `repo.claim.unknown` and `repo.site.mismatch`.

**The secret.** `uses-ink-authority-repo` draws 32 bytes, gives them to the control plane as the secret and returns them once,
as 64 hex digits in a `Secret` that prints as `Secret(<redacted>)`, is zeroed on drop and compares in constant time. No call
shows it again, and the control plane opens it only to check a delivery. Rotation is how a lost one is replaced.

**The GitHub proof.** The App's installation (0110) is the strong proof and needs an App an operator registered. A
self-host has none, so `Open::prove` reads the same challenge file through the contents API with no credential, for a
public repository, as Forgejo and GitLab do (0111, 0112). A private repository is `repo.proof.missing`.

**The delivery names its repository.** Each forge's `Hooks::named(body)` reads the repository from a delivery before it is
trusted, so the secret of its claim can be found. Nothing it returns is acted on until the signature held. `Push` carries the
default branch the event names, which `Watch::pushed` needs (0161).

*Left:* the GitHub App itself and the OAuth steps are owner work. A claim of a repository on a host no forge lists is
refused at the proof, not at the claim, since the claim only compares the site's own address.
