---
number: "0104"
title: "An organization has a typed one-line name and a roster that decides what a member may give a key"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0104 An organization has a typed one-line name and a roster that decides what a member may give a key

Built (2026-11-28).

- **`OrgName`.** One line, trimmed, not empty, at most 64 characters, no control characters. It is a type
  with a typed error (`NameError`), parsed at the edge (`AddOrg`), the form and the CLI, never a plain string.
- **`Roster`.** The Door (the approve page) asks a `Roster` what role a person has in an organization. A key
  approved with `org` is made with `Control::approve_as`: its account is the organization, its rights are
  the approver's rights cut by the role, and it stays when the approver is removed. `GET /v1/orgs/{org}/keys`
  says per key whether the approver is still a member (`creator_removed`).
- **Typed refusals.** `Seated` names why a roster refused: not a member, a role short of the right, or the store.
- **Left.** The CLI has no verb to approve a key for an organization; the approve page of the dashboard does not
  offer the choice yet. The Door accepts `org` in its POST body.
