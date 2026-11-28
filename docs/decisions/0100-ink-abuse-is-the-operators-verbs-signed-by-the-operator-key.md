---
number: "0100"
title: "`ink abuse` is the operator's verbs over the API's operator routes, signed by the operator key"
status: built
date: 2026-11-28
area: cli
supersedes: []
---

## 0100 `ink abuse` is the operator's verbs over the API's operator routes, signed by the operator key

Built (2026-11-28), the CLI half of decision 0089: `ink abuse list`, `ink abuse suspend <SITE>` and `ink abuse
reinstate <SITE>`. The routes existed (`GET /v1/abuse`, `POST /v1/sites/{site}/suspend`, `.../reinstate`); nothing
in `ink` called them, so an operator had to sign requests by hand.

- **No second credential.** The verbs sign with the key `ink login --data` made, which is a line of the server's
  `operator-keys` file. A device key or a deploy key is refused by the route (`Call::operator`), as before. The
  verbs take the same `--api` and `--timeout` as `ink key`, and have no `--site`: a suspension names its site as an
  argument, since it is about another account's site.
- **Parsed before sent.** The site and the report are typed ids and the kind is `control::AbuseKind`
  (`phishing`, `malware`, `illegal`, `impersonation`, `spam`, `other`; `FromStr` with a typed `UnknownAbuse`), so a
  misspelt kind is a usage error (exit 2) that names the kinds, and nothing is sent.
- **Budgets need no new flags.** Decision 0088's limits already have `--limit-*` flags and `INK_LIMIT_*`
  variables on `ink serve`, and the report rate has `--abuse-limit`; this closes that part of the wiring check.
- **A fault found by the end-to-end test.** The edge's axum adapter had no arm for `Status::Forbidden`, so a
  suspended site's notice went out as `500`. It is `403` now, which is what 0089 says.

**Not done.** A mail to the owner when a site is suspended, and a way to read the suspensions apart from the
reports (a suspension without a report is visible only as the site's refusal). **Reversal:** remove the verb;
the routes do not change.
