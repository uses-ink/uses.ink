---
number: "0087"
title: "A handle a site leaves is held for twelve months, a verified owner's handle for good, and the brand list is a data file"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0087 A handle a site leaves is held for twelve months, a verified owner's handle for good, and the brand list is a data file

Built (2026-11-28) with the owner's defaults of `docs/hosted.md` HQ11 (package C1, migration 301). It
replaces the permanent tombstone of 0022 and architecture section 12 with a hold that ends.

- **A `hold` table, not new columns.** Migration 301 `Held` makes `hold` (handle, site, why, until,
  at). The numbered migrations before it are not edited, so a database that has them upgrades by one
  step. `tombstone` stays and is still written on removal; the claim path reads `hold` first and a
  tombstone with no hold row (one made before 301) counts as a removal held from its day for the
  operator's hold, so no old handle becomes free the day this ships.
- **Why a handle is held** is `HoldReason`: `Removed` and `Renamed` end after the hold (default 365
  days, `--handle-hold <DAYS>` or `INK_HANDLE_HOLD`); `Verified` has no end. A claim after the end
  deletes the hold and the tombstone in the same batch that adds the site, so the unique key on the
  handle still decides a race.
- **"Verified" is a recorded fact**, `Control::verify(site)`: it writes a `Verified` hold for the
  site's handle and a `handle.verified` event. No code calls it yet; the forge authority of M4 will,
  when it has proved the person owns the account. A removal leaves a verified hold as it is, so a
  verified handle is never reclaimed. Verifying a handle while its site is alive does not make a claim
  of it answer `Held`: a live site answers `Taken` first.
- **`Control::rename_site`** gives a site another handle under the same checks as a claim and holds the
  old one as `Renamed`, written only when the site really left it, so a rename is not a way to free a
  name for someone else at once. Verification belongs to a handle, so the new handle is not verified.
- **The brand list is `crates/server/control/data/reserved-handles.toml`**, one list of names read once
  (`Brands::bundled`) and checked: every name must be a handle and none may be an infrastructure name.
  A list that does not read is `Error::Brand`, not an empty list. The names in it are a short
  placeholder the owner replaces. `Handle::reservation` says which list a name is on.
- **Refusals have codes.** `Refusal::Tombstoned` is now `Refusal::Held { handle, why, until }`, and
  `Refusal::Reserved` says which list. Every refusal of the control plane now has a code of the catalog
  (`Refusal::code`) and its data (`Refusal::fields`), and the API answers with them in place of the
  word `control`: `handle.name.reserved`, `handle.name.brand`, `handle.name.taken`, `handle.name.held`
  (with `until`) and `handle.name.kept`.
- **`control::Schema::after`** lists the migrations after the queue's in order, so a harness or a server
  names one list and each package of M4 adds a line to it. `Policy` carries the operator's settings of
  the control plane (here the hold; the budgets join it), and `Control::with_policy` applies it.

**Reversal:** a hold that is too long or too short is a flag. A name taken from a person who held it is
not recoverable, which is why a verified handle has no end.
