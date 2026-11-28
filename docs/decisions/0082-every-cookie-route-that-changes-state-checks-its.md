---
number: "0082"
title: "Every cookie route that changes state checks its origin, and starting a passkey sign-in is limited per source"
status: built
date: 2026-11-28
area: security
supersedes: []
---

## 0082 Every cookie route that changes state checks its origin, and starting a passkey sign-in is limited per source

Built (2026-11-28). It closes two "not done" items of 0068 (slice 2): the Origin check on the older
`Door` routes and a limit on `POST /v1/passkeys/challenge`.

- **Origin on the `Door`.** Every route of the `Door` that is not a `GET` (`POST /v1/login`,
  `POST /v1/login/{token}`, `DELETE /v1/session`, `DELETE /v1/sessions/{id}`, and the approve routes)
  needs an `Origin` that is one of the allowed origins exactly, or, with no `Origin`, `Sec-Fetch-Site:
  same-origin`; with neither it is refused `403 auth.origin.refused`, as `docs/auth.md` section 8 says.
  A refused request changes nothing: the link is not spent and the session is not ended.
- **Which origins.** The `Door`'s configuration has no field for them and `ink serve` builds it by a
  struct literal, so the default is read from what the `Door` already holds: the origin of the link
  base (`--login-base`, by default the API's own address), which is where the page that asks for a link
  and spends one is served. `Door::with_origin` adds more, such as a dashboard on another host. There is
  no `ink serve` flag for it yet (see "Left"). One type, `Origins`, holds the list and the rule.
- **The one exception** is `GET /v1/login/{token}` with `--login-get-signs-in`: that GET signs in by the
  operator's choice, for an instance with no page in front, and has no browser to ask.
- **Challenge limit.** `POST /v1/passkeys/challenge` takes no credential and writes a row, so it is
  limited per source: 60 an hour by default, `Quota` (the sliding window of the sign-in limiter, now
  shared), `429` with `Retry-After` and the new code `auth.rate.limited`. A request with no known source
  shares one bucket, so a missing address is no way round. `Ceremonies::with_challenges` sets another
  rate. Behind a reverse proxy every request has the proxy's source until 0084's forwarded-header setting
  is on, and then the limit is one for everybody behind it: 60 an hour is enough for a household and a
  reason to turn that setting on.

**Left (the CLI's, not done here).** `ink serve` has no flags for an extra origin or a challenge rate;
the defaults above apply. **Why:** a cookie is sent by a browser on a cross-site form post, and `Lax` is
the first line of defence, not the only one. **Reversal:** a deployment that fronts the API with a page on
another host adds that origin with `Door::with_origin`; the check cannot be turned off.
