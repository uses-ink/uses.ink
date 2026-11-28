---
number: "0161"
title: "A push queues its build through the dedupe the poll uses, and a signed webhook is the one route that is not signed by a key of ours"
status: built
date: 2026-11-28
area: edge
supersedes: []
---

## 0161 A push queues its build through the dedupe the poll uses, and a signed webhook is the one route that is not signed by a key of ours

Built (2026-11-28). The route of 0113.

**`Watch::pushed(site, push)`.** A push to the branch the site builds queues a build with the trigger `push`, the dedupe key
the poll uses (`poll-{site}-{commit}`), and records the commit in the `seen` table. The poll's `settle` reads that row
before it queues, so a push and the poll that follows it make one build even when the first has been built and has left the
queue, which the queue's own dedupe does not cover. The branch is the one the site's revision names, else the repository's
default branch as the event says it; an event that names none leaves a site that follows the default unplaced, with the
typed reason `Unplaced::Default`. A push to another branch is a preview through the pin table (0102), under the same cap
and the same public-source rule; with previews off it is `Unplaced::Previews`, a reason and not a failure. It takes `&self`
and needs no remote: `Watch::pusher(control, clocks, previews)` makes the half of a watch that takes pushes, so the API's
task holds one beside the poll loop's, and the two share only the ports.

**The route.** `POST /v1/hooks/{github,forgejo,gitlab}` is behind `--forge-hooks` (`INK_FORGE_HOOKS`), off by default; off,
the path is no route. It is the one route that no key of ours signs: the forge signs the body with the secret of the claim.
The order is the body (read within `--forge-body`, 5 MiB by default), the repository the body names, the claim of that
repository (a repository nobody proved is answered as a wrong signature is, so a stranger cannot learn which repositories
are followed), the secret from the secret port, then the forge crate's own checks in its order: size, signature, event, and
the JSON. Only then is the delivery id claimed in the `Nonces` store, keyed by a hash of the forge and the repository, with
the end of the window as its expiry. That is the replay memory: a delivery seen inside `--forge-window` (600 s) is `409`
`hook.delivery.replayed`, and the store takes up to `--forge-remembered` ids for one repository at once. The window in
`Intake` lives for one delivery, because the secret is per claim and a `Hooks` is made per delivery; the store is what
remembers, and on a Worker it is the Durable Object of 0135. A push that is taken is `202` with `queued`, `duplicate`, `left`
or `ignored`; the dedupe on site and commit makes a replay with a fresh id harmless.

**Headers.** `Request` has a `headers` field that the adapter fills for this route only, from every header but
`Authorization` and `Cookie`, which have fields of their own. Its `Debug` prints names, never values.

*Left:* the memory store loses its ids on a restart; the dedupe is what keeps a replay after one harmless. Self-host runs
the route on the API host; the edge's other host is not asked.
