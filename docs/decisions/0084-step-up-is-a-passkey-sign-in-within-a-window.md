---
number: "0084"
title: "Step-up is a passkey sign-in within a window; adding or removing a passkey and approving a key need it"
status: built
date: 2026-11-28
area: security
supersedes: []
---

## 0084 Step-up is a passkey sign-in within a window; adding or removing a passkey and approving a key need it

Built (2026-11-28). It closes the step-up item of 0068 (slice 2): `docs/auth.md` section 8 says anything
that adds or widens a credential needs a passkey assertion within the last five minutes, and a step-up
replaces the session id.

- **A step-up is a passkey sign-in, and needs no new route.** `POST /v1/passkeys/challenge` and
  `POST /v1/passkeys/session` already make a session from a passkey assertion with user verification.
  That session is the proof of presence: `SignIn::stepped` is true when the account has a passkey and a
  passkey made the session (its `passkey_session` row) no more than the step-up window ago. The window
  is `SignIn::STEP_UP`, five minutes; `SignIn::with_window` sets another (a `Ttl`, one second to 366 days).
- **The session id is replaced, as auth.md asks.** A passkey sign-in that carries a session cookie ends
  that session, so the old secret stops working the moment the new one is set. `docs/auth.md` names a
  `stepped_up_at` column for the same fact; `db` has no `ALTER`, and `created_at` of a passkey session
  is that time, so no column is added.
- **Gated:** `POST /v1/passkeys/options` and `POST /v1/passkeys` (adding a passkey, checked at both
  steps, so a ceremony started in time is not finished late), `DELETE /v1/passkeys/{id}`, and
  `POST /v1/approve/{code}` (approving a device or deploy key). Not gated: any read, looking at an
  enrolment, turning one down, ending a session, and a device key revoking keys (0068: it can revoke, not
  mint).
- **Exempt: an account with no passkey.** There is nothing to prove it with, and a session made by the
  first-run or sign-in link is as strong as the link; without this the first passkey could never be
  added. The moment the account has a passkey, a session an email link made no longer steps up: the
  person signs in with the passkey, which is what the dashboard script does by itself (it catches
  `auth.step-up.required`, runs the sign-in and retries the action once).
- **The refusal** is `403` with the new code `auth.step-up.required`, naming the window, on both the
  passkey routes and the `Door`'s approve route.
- **Not built:** step-up for changing recovery settings and for starting an account's deletion, because
  neither exists yet (0085 adds recovery codes, which are gated here).

**Left (the CLI's).** `ink serve` has no flag for the window; the five minutes apply. **Why:** a stolen
session cookie, a leftover browser tab or a link read by a mail scanner must not be enough to give an
attacker a credential of their own. **Reversal:** the window is one setting; the exemption is one test
in `SignIn::stepped`.
