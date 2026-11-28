---
number: "0113"
title: "Forge webhooks reach the watch through one route that is not wired yet, and the proofs bind in the control plane"
status: accepted
date: 2026-11-28
area: control
supersedes: []
---

## 0113 Forge webhooks reach the watch through one route that is not wired yet, and the proofs bind in the control plane

The three authority crates (0110 to 0112) are built and tested. The route that feeds them to the watch is not, because
it is more than a day of work and not small: it needs four things that do not exist. This decision is the plan.

- **The route.** `POST /v1/hooks/{forge}` with `{forge}` one of `github`, `forgejo`, `gitlab`, behind a setting that is off by
  default. It reads the body within the configured limit, builds the forge's `Headers`, builds a `Roster` from the sites that
  have a git source on that forge, calls the forge's `Hooks::receive`, and answers `204` for a push or an ignored delivery
  (the typed reason goes to the log as the `hook.event.ignored` notice) and `4xx` with the refusal's catalog code otherwise.
  A refusal never says which part of the signature failed.
- **What it needs first.** (1) A lookup from repository to sites. `control.watched()` lists every git site, so the route can
  filter by locator now, but per delivery that is a scan; an index on the normalised repository comes with the binding below.
  (2) The webhook secrets: one per forge on self-host, set by the operator, and per instance for Forgejo and GitLab. They
  come from the secret port, never a file in the repo. (3) A way into the watch: `Watch::settle` is private to the poll
  loop and keeps the commit it saw in its own table. A webhook needs `Watch::pushed(site, head)` that shares that table and the
  build dedupe on site and commit, so a push and the poll that follows it queue one build. The trigger is `Trigger::Push`,
  which exists. (4) A shared replay memory: the window in `Intake` is per process, which is right for self-host and wrong
  for a Worker, where it lives in a Durable Object.
- **Binding a proof to a site.** The authorities return a typed `Proven { repo, owner }` or `Covered { repo, installation,
  account }`. As 0093 does for a domain, the control plane holds a claim (the challenge and the site) in a table and decides
  what a proof means: a `repo_claim` table with the site id, the normalised repository and the challenge, a unique key on the
  repository once proven, released with the site. That is a migration and `Control` methods, and it is where "first claim wins"
  across forges is enforced.
- **A real `Transport`.** The port has a fake only. The self-host adapter is a small `reqwest` client over the one rustls
  configuration of `uses-ink-https` with the timeout and reply limit of the request; the Worker's is `fetch`. Neither follows redirects.
- **Owner steps.** Registering the GitHub App, a Codeberg application and any GitLab application, and storing their secrets
  through `secrets_request`. None was done.
