---
number: "0089"
title: "A reported site is suspended first, as a row of its own, and anyone may report with no account"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0089 A reported site is suspended first, as a row of its own, and anyone may report with no account

Built (2026-11-28), with the owner's default of `docs/hosted.md` HQ10: suspend first, delete later (package C4).

- **A report names a handle and is unsigned.** `POST /v1/abuse` takes `{handle, kind, detail, contact?}` from
  anyone and answers `201 {id}`. It stores the handle and not a site, so a report outlives its site and the
  answer says nothing about whether the handle is one. `detail` (2000 characters) and `contact` (200) are bounded
  text with no control characters; a body that breaks a bound is refused whole as a `400`. Kinds are
  `phishing`, `malware`, `illegal`, `impersonation`, `spam` and `other`.
- **Per source, like the passkey challenge.** The report route counts with the sign-in's `Quota` (an IPv4
  address or an IPv6 /64, one bucket for unknown sources), five an hour by default, `--abuse-limit <COUNT/WINDOW>`
  or `INK_ABUSE_LIMIT`. Over it the answer is `429` with `Retry-After` and the code `abuse.report.limited`.
  A refused body still counts. Behind a proxy the source is the proxy until 0086's forwarded header is set.
- **Suspension is a row, not a site state.** `suspension` (site, reason, operator label, report, time) is
  migration 303 beside `abuse_report`. A state would need a new value in the site's check, and a migration
  cannot change one; a row also makes reinstating a delete. The operator calls `POST /v1/sites/{site}/suspend`
  `{kind, report?}` and `POST /v1/sites/{site}/reinstate` with a key of the operator file; `GET /v1/abuse` lists
  reports. Any other key is `403 auth.operator.required`. The site's log gets `site.suspended` and
  `site.reinstated`, and the report becomes `actioned` in the same batch.
- **What a suspended site does.** `Control::serving` gives nothing, so the edge asks `Sites::suspended` and
  answers `403` with a text notice that names the reason and carries no byte of the site, `no-store`. Deploys and
  build requests are refused as `site.state.suspended`. Files and rows stay, so a reinstated site is as it was.
  The owner can still export it and remove it; removing deletes the suspension row, and the handle is held as
  after any removal (0087).
- **The edge asks twice only for a host that serves nothing.** A suspension is not cached: it is read from the
  control plane for each request to a host that has no live deploy, so a lift or a suspension applies at once
  for hosts that were not just served. A host the edge remembers as live is answered from the pointer cache until
  its time to live ends (`Settings::ttl` of the edge lookups), which is the longest a suspension can wait to take
  effect.
- **Not done.** Notifying the reporter or the owner, dismissing a report without suspending, and a dashboard
  screen for reports (the operator uses the API).

**Reversal:** a `state` column for suspensions needs a table rebuild; the rows migrate with one statement.
