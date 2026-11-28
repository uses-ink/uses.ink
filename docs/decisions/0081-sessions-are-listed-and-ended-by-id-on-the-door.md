---
number: "0081"
title: "Sessions are listed and ended by id on the Door, and the dashboard ends one with a form post"
status: built
date: 2026-11-28
area: security
supersedes: []
---

## 0081 Sessions are listed and ended by id on the Door, and the dashboard ends one with a form post

Built (2026-11-28). The owner has not read it. It closes the first "not done" item of 0066 (a route to
list and revoke sessions) and the session list of `docs/auth.md` section 8.

- **Two routes on the `Door`**, beside `/v1/session`, reading the session cookie like the rest of it:
  `GET /v1/sessions` lists the account's live sessions, newest first, with the asking one marked
  `current`; `DELETE /v1/sessions/{id}` ends one. The row is the `Session` of `control::signin` (id,
  created, last used, expires, limit). **No secret is in a list**: a session's id names it and signs
  nothing, and the secret exists only in the cookie.
- **One answer for what is not yours.** Another account's session, an ended one, an id that was never
  made and text that is no id are all `404` with `auth.session.missing`. Ending the asking session
  clears its cookie, as `DELETE /v1/session` does.
- **The dashboard needs no script.** `docs/auth.md` says sessions are managed on the dashboard, and a
  form cannot send `DELETE`, so the Account screen posts to `/account/sessions/<id>` and the edge's
  `Screens::end_session` answers `303` back to the account, the pattern the site actions use. The one
  script stays `/passkey.js`, for WebAuthn only.
- **Left out of auth.md's list:** a label such as "Firefox on Linux". It needs a user agent parsed at
  sign-in and a column the tables have no way to add (`db` has no `ALTER`); the list shows the dates,
  which is what tells a person which session is theirs. A session ended by removing its passkey is
  already ended (0068); "end every other session" is the same route in a loop.

**Why:** a stolen cookie is only recoverable if its owner can see and end sessions they do not
recognise, and the list is the cheapest control that gives that. **Reversal:** none needed; the routes
are additive.
