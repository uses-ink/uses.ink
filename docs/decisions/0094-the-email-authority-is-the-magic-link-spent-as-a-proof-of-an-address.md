---
number: "0094"
title: "The email authority is the magic link spent as a proof of an address, under its own digest key"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0094 The email authority is the magic link spent as a proof of an address, under its own digest key

Built (2026-11-28), package C8 of M4: `ink:authority-email` (decision 0013), the one-time link of decision 0066
as a proof that someone reads mail at an address. It needs no migration. An organization invitation and a
recovery step are its callers; neither exists yet.

- **Same table, other key.** A proof is a row of `challenge` like a sign-in link, but its digest is made under
  `Context::Authority`, not `Context::Challenge`. A sign-in link therefore finds no row when presented as a proof,
  and a proof finds none when presented as a sign-in (`Refused::Unusable` either way, which does not say why, as
  before). A failed attempt of the wrong kind spends nothing.
- **No session, no account.** `SignIn::vouch(address, proof, (ttl, now))` stores the row with no account; `SignIn::attest`
  spends it with the same conditional `UPDATE ... WHERE used_at IS NULL AND expires_at > now` as a sign-in, and
  returns the address. Of two callers that present one proof, the database lets one win.
- **Fifteen minutes, once.** `Email::LIFE` is 15 minutes, the default of a sign-in link (`Settings::login`), and
  it is a separate value so the two can differ. The row ends on the same `expires_at` and is swept with the others.
- **The purpose column says `login`.** The check of migration 200 allows `login` and `admin` and nothing else, and
  `Purpose::NAMES` feeds it, so a new purpose would break databases that already have the table. The digest key is
  the tell, and the column is not read to decide what a row is for.
- **A new mail template.** `Template::Prove` in `uses-ink-mail` (outside this package's files, one variant): the
  subject "Confirm this address" and a body that says the link does not sign anyone in.
- **A failed mail leaves the proof stored.** It ends on its own; nothing is rolled back, because nothing outside
  the row depends on it.

**Not done.** No route calls it yet: the confirm page and the invitation that sends a proof belong to the orgs
work. **Reversal:** drop `Context::Authority`, `vouch`, `attest`, the crate and the template
variant; no data is left behind that a sign-in reads.
