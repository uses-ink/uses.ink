---
number: "0092"
title: "The dashboard is served on the API host by ink serve, with a derived origin and a per-request site lookup"
status: built
date: 2026-11-28
area: control
supersedes: []
---

## 0092 The dashboard is served on the API host by ink serve, with a derived origin and a per-request site lookup

Built (2026-11-28). `ink serve --data` mounts the web dashboard (`crates/surface/web`) beside the API. It closes the
"nothing implements `Screens`" gap of 0082 to 0085.

- **One host.** `/v1` and `/v1/...` are the API; every other path on the API host is a dashboard page. The session
  cookie has no `Domain`, so it belongs to the host that set it. A passkey belongs to the relying party id, and
  `/passkey.js` fetches `/v1/passkeys/...` with relative URLs. Pages and routes therefore share a host, and nothing
  else lets the cookie, the passkey and the Origin check of 0083 agree. A separate dashboard host is a proxy setup:
  point `--login-base` and `--rp-origin` at it and route `/v1` to the API.
- **No second copy of the sign-in rules.** The `Screens` implementation (`crates/surface/cli/src/selfhost/dash`) reads
  sites, builds and deploys from the control plane. Everything about who is signed in goes through the API's own
  routes, called as the browser that asked: its cookie, its `Origin`, its source address. Rate limits, the outbox,
  enumeration-flat answers and revoked sessions are the ones the API already has. The layer rule holds: server crates
  do not depend on `web`, so the implementation lives in the CLI.
- **CSRF.** Every state change is a `POST` form. A `POST` page is refused with 403 `auth.origin.refused` unless
  `Origin` is an allowed origin or the browser says `Sec-Fetch-Site: same-origin`, by the same `Origins` as the door
  (0083). A `GET` changes nothing, so a mail scanner opening a link signs nobody in: the page of a link has a form
  that posts. Cookies are `HttpOnly`, `SameSite=Lax`, `Secure` unless the origin is local http. The token of
  `/login/<token>` is redacted from the request log like `/v1/login/<token>`.
- **The link points at the dashboard.** The default `--login-base` is `<api host>/login`, not `/v1/login`.
  `/v1/login/<token>` still works for a client that is not a browser. `signin` and `v1` are reserved handles, since a
  handle is the first path segment of a site's screens.
- **Replies carry headers.** `edge_api::Reply` has `headers`, `Status::SeeOther` and the media `Html` and `Script`, so
  a page can redirect and set its own security headers; the bridge applies them.
- **Origin is derived.** `--origin` has no default. Without it the server binds first and uses
  `http://localhost:<port>` of the address it got, and the start line says so (`from the listen address`). The host
  is `localhost` even when listening on `0.0.0.0`, because WebAuthn rejects an IP as a relying party id. The flag stays
  authoritative.
- **No stale site.** `--site-ttl` now defaults to 0: the lookup asks the control plane on every request. They share
  a process and a database, so a deploy or rollback showed up late for nothing; the old 10 s default made a fresh
  push look unserved. Raise it to spare the database a read per request.
- **Account read.** `Control::account(id)` reads an account's creation time for the Account screen.
- **Not done.** The log of a running build has no live source in self-host (the log is stored when the build ends),
  so a running build shows a refreshing page without lines. The Understood screen does not read back what the source
  lacked, the Site screen shows problems from the stored log as raw lines, `why` has no replay and says so, and the
  email recovery of 0085 is still open.
