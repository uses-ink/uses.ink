---
number: "0095"
title: "An org is an account with members whose roles are code; the last owner is held by one statement"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0095 An org is an account with members whose roles are code; the last owner is held by one statement

Built (2026-11-28), package C5 of M4, migration 307: organizations and roles as `docs/auth.md` section 12 sets
them. The calls of the HTTP API do not yet act for an organization (see Not done).

- **An org is an account.** `org` has a row per organization whose id is an `account` row, so the organization
  owns sites (`site.account_id`), keys (`key.account_id`) and audit events like a person's account does. `member`
  holds (org, person, role, joined), keyed by the pair.
- **Roles are code.** `Role` is Owner, Admin, Writer, Reader, and `Role::rights` is the table of auth.md: Owner has
  every right, Admin every right but `Destroy`, Writer `Read`, `Deploy` and `Edit`, Reader `Read`. The column's
  check is built from `Role::NAMES`, so a row cannot hold a role the code does not know, and no custom role
  exists. Admin lacks `Destroy` as a whole: a right is not split by what it destroys in v1, so an admin cannot
  remove a site either. This is stricter than "all but Destroy of the org" and widening it is one line.
- **Who manages whom is one function.** `Role::manages(other)`: the manager needs `Members`, and only an owner
  makes or unmakes an owner. So an admin cannot grant Owner, cannot raise themselves and cannot remove or demote an
  owner. The tests check it for every actor, target and new role.
- **The rules sit in the statement.** `add_member` is `INSERT ... SELECT ... WHERE EXISTS (the asker's seat with a
  role that manages the new one)`. `set_role` and `remove_member` are one `UPDATE` or `DELETE` whose `WHERE` holds the
  asker's seat and "this row is not an owner's, or another owner exists". A person who is demoted between a read and
  the write therefore changes nothing, and two owners who remove each other at once cannot both succeed: whichever
  statement runs second finds no other owner. When a statement changes no row, a second read says why (not a member,
  the asker's role, or the last owner) for the refusal. A member may always leave, unless they are the last owner.
- **A key outlives its approver.** `Keys::approve_as` writes a deploy key whose account is the organization and
  whose `approved_by` is the member; `Control::approve_for` allows it to a member whose role has `Deploy` and
  gives the key no more than that role. Removing the member deletes the `member` row and nothing else.
  `Control::org_keys` lists the organization's keys with `creator_removed` for the ones whose approver is no longer a
  member; an owner decides whether to revoke them. A device key cannot belong to an organization.
- **A person's grants.** `Control::grants(person)` is one `Account(org)` grant per membership, with the role's
  rights. Keys stay attenuations of those, as before.

**Not done, and why.** The HTTP API still resolves a caller to one account: the `Target` of a call is the
principal's own account, `owned()` checks `site.account == principal.account`, and a device key's scope is its
person's account. Acting for an organization needs those three changed together (the grants above as the person's
rights, the site's account as the target, the key's scope), and the routes to make an org and manage members. Doing
one without the others would give a member a right on a site the ownership check then denies, or the reverse, so
none is changed here. The Account screen lists a person's organizations when given them; invitations by the email
authority (0094) are the next caller. **Reversal:** drop `org` and `member`; nothing else refers to them except
keys whose account is an organization.
