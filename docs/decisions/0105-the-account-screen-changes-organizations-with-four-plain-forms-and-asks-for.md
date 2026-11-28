---
number: "0105"
title: "The Account screen changes organizations with four plain forms and asks for the step-up before it changes members"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0105 The Account screen changes organizations with four plain forms and asks for the step-up before it changes members

Built (2026-11-28).

- **Four POST forms, no script.** `/account/orgs` (make one), `/account/members` (add), `/account/roles` (regrade)
  and `/account/removals` (remove). They are read by `Seating::read` into a typed change, answered by a redirect
  to `/account`, and Origin-checked by the Hosted mount like every other form. A `GET` of them is a 405.
- **Step-up.** A change of members asks the API's `GET /v1/session/fresh` first, so the dashboard has no second copy
  of the rule: an account with a passkey needs a passkey sign-in within the window, an account without one does
  not (`docs/auth.md` section 8). Making an organization needs no step-up.
- **Ids, not search.** A member is named by account id, as the API and the CLI do. The card of each organization
  shows its id and its members' ids, so one can be copied. No address lookup exists, to avoid telling anyone
  whether an address has an account.
- **Refusals** are the control plane's: `org.member.exists`, `org.owner.last`, `org.role.denied`, each as a page
  with a fix line.
