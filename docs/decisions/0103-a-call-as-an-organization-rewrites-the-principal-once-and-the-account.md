---
number: "0103"
title: "A call as an organization rewrites the principal once, and the account, the site check and the key reach change together"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0103 A call as an organization rewrites the principal once, and the account, the site check and the key reach change together

Built (2026-11-28), the reach of decision 0095.

- **The route says it.** `/v1/orgs/{org}/sites`, `keys` and `whoami` are the account routes made as the
  organization. `Call::acting` reads the organization from the path; the handlers are the ones a person's own
  calls use.
- **One rewrite.** `Api::acting` returns a `Principal` whose `account` is the organization and whose `effective`
  grants are the key's own joined with the organization's grant for the person (`Effective::joined`). The site
  `owned()` check and the key's scope read the same principal, so they cannot disagree: this is the reason both
  change together, and why no handler picks an account by itself.
- **Who may act.** A person's key (the operator, or a device key) reaches the organizations the person is a
  member of, with what both the key and the role allow. A deploy key reaches only its own organization. Anyone
  else finds no organization (`org.id.unknown`, 404), the same answer as an id that never existed.
- **Members are not an acting call.** `members`, `PUT` and `DELETE` on a member use the person as the actor
  (`Principal.person`) and need the `Members` right, which a device key does not hold by default.
- **Leave is not separate.** Removing yourself needs the `Members` right like any removal, so a reader cannot
  leave on their own. A leave route is left for later.
- **`--org`.** `ink init|push|status|...` take `--org <ID>` (`INK_ORG`); the client puts `/orgs/{org}` before the
  account routes. `ink org create|list|member add|remove|role|list` call the member routes.
