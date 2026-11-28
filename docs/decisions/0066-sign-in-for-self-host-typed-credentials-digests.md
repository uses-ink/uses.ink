---
number: "0066"
title: "Sign-in for self-host: typed credentials, digests at rest, a mail port, links that look before they spend"
status: built
date: 2026-11-27
area: control
supersedes: []
---

## 0066 Sign-in for self-host: typed credentials, digests at rest, a mail port, links that look before they spend

Built (2026-11-27) as M3's sign-in: `control::signin`, `mail` and `crates/port/mail/smtp`, and `crates/server/edge/api`'s
`Door`. The owner has not read it yet; these are the choices to push back on. The dashboard
screens are the `web` branch's and call these routes (the seam is the last bullet).

- **No password exists, and nothing reversible is stored.** An account's ways to sign in are the
  typed `signin::Credential` enum, `#[non_exhaustive]`; the only variant is `Link { address }`
  (stored in a `factor` row with a `kind`). A passkey and a device key are later variants and
  are not built. A one-time token and a session secret are 256 random bits (`Proof`, two draws of
  `clock::Draw`) and the rows hold only their BLAKE3 digest, keyed per use (`Context::Challenge`,
  `Context::Session`), so a link is no session and the reverse. No salt or stretching: the input
  is 256 random bits. A digest is compared in constant time (`blake3::Hash`), though the lookup
  already names the row. `Proof` is a `secret::Plain` inside: `Proof(<redacted>)` in `Debug`,
  zeroed on drop, no `Clone`. The `trace` crate had not landed, so `secret::Plain` is the
  redaction type; when `trace` lands, `Proof` keeps its own `Debug` and the seam is its `reveal`.
- **`SignIn<D: Db>` is generic over the row port alone**, not `Control<P>`, so a credential check
  holds one without the blob, queue and secret ports and `Ports` keeps its four. It reads no
  clock and draws nothing: `Drawn` (a proof and three ids) and the time are arguments. Tables
  (`factor`, `challenge`, `session`) come from the typed migration `Entry` (number 200, after the
  queue's 100); no SQL text.
- **A link is single use by one conditional `UPDATE ... WHERE used_at IS NULL AND expires_at >
  now`**; of two callers, the database lets one change the row. Every refusal that can be known
  first (policy closed, address taken) is made before the update, so it does not spend the link.
  If the sign-in's own rows then fail to write, the link is spent and the person asks again. Used,
  expired, never made and tampered are one `Refused::Unusable`, answered `404` in one shape; a
  token that is not 64 lower-case hex digits gets the same answer as one that was never made.
- **`GET /v1/login/{token}` looks and `POST` spends.** The roadmap line says both redeem. A mail
  scanner or a link preview opens every link in a message with a `GET`, which would spend a
  single-use link before its owner clicks it, so `GET` returns what the link is for and when it
  ends, and `POST` signs in. `Config::reveals` (`--login-get-signs-in`) makes `GET` sign in for
  an instance with no web page in front.
- **The first-run link** is a `challenge` row with purpose `admin` and a unique `slot`, so it can
  be made once; the plaintext is returned by that one call (`ink serve --data` prints it once)
  and the row holds the digest. It is not made when any account has a way to sign in. It signs in
  the fixed owner account of 0052 (the account stays, so the owner token and the link name one
  person) or makes an account when none is given; `{"address"}` in the body gives that account a
  `Link` credential, so the owner signs in by mail afterwards. It ends after `--admin-ttl` (24 h);
  a link that ended unused is not made again, and the owner token still works (the way back in is
  a deliberate later `ink` verb, not built).
- **Who may make an account** is `Signup`: `first` (default), `open`, `closed`. `first` lets an
  address make the account while no account can sign in and no first-run link was made, so on a
  fresh public instance the first-run link, not the first visitor, gets the instance. An address
  that has an account always signs in.
- **Sessions** are rows with a digest, an id (to list and revoke; it signs nothing), `expires_at`,
  `limit_at` and `revoked_at`. `Expiry::Sliding { idle, limit }` (default 7 days idle, 30 days
  at most) moves `expires_at` on each use, never past the limit; `Expiry::Absolute { ttl }` does
  not move. `end`, `revoke`, `revoke_all` and `sessions` (per account) are use cases; expired
  rows are swept on each request for a link. Every lifetime is a `Ttl` (1 s to 366 days, read
  from `15m`, `12h`, `7d`).
- **The `mail` port** is `Mailer` (async, not `Send`), a validated lower-case `Address`, a `Base`
  for links and a `Message { to, Template }`. The template is an enum, so the subject is
  `Template::subject()` and the body is its `Display`; no call site formats text. `Link` prints as
  `Link(<redacted>)`. `Capture` keeps messages for tests; `Stdout<W>` writes them to a writer for
  a dev run (this crate prints nothing itself); `crates/port/mail/smtp` is `lettre` with rustls, a
  `Settings` of host, port, `Security` (`starttls`, `implicit`, `plain`), an optional `Login` and
  the from address. The password is a `secret::Plain` read from an environment variable or a
  file, never a flag, and is in no `Debug` or error.
- **Mail is not sent in the request.** `POST /v1/login` puts the message in a bounded in-memory
  `Outbox` and answers `202 {"requested":true}`, one value built before the outcome is known; a
  `Courier` on the API task sends. The request's time then differs between a known and an unknown
  address by one local row write, which is below network jitter: flat enough against a remote
  observer and not constant time against one on the machine. A full outbox drops the message,
  counts it and the courier says so; a mail lost to a restart is a link the person asks for
  again. Rate limits (`Limiter`, `Rates`: per address 5/1h, per source 20/1h, 10 000 tracked,
  all flags) count every request alike, so a `429` says nothing of whether the address exists. A
  source is an IPv4 address or an IPv6 /64; behind a reverse proxy it is the proxy's, and
  trusting a forwarded header is not built.
- **One `Credentials` seam.** `Sessions` is a `Credentials` adapter over `SignIn`; `Either<A,
  B>` asks one and then the other, so the API takes the owner token (the operator credential,
  unchanged) or a session, and a later passkey or device-key adapter is another `Either` arm built
  where the API is made, with no caller changed. `Door` and `Routed` put the sign-in routes
  beside the API; `Request` gained `cookie` and `source`, `Reply` gained `cookie` and `retry`.
- **The cookie** is `HttpOnly`, `Secure` unless the instance is plain `http` on a loopback name,
  `SameSite=Lax`, `Path=/` (configurable), no `Domain`, `Max-Age` of the session's limit; the
  default name is `__Host-ink-session` when secure and `ink-session` otherwise (a `__Host-` name
  without `Secure` and `Path=/` is refused at start). The cookie is read by the sign-in routes
  only; the API's own routes read the bearer, so a cross-site form cannot act with the cookie.
- **No secret in a log.** `Request` and `Reply` have hand-written `Debug` (headers as
  `<redacted>`, the body as a length); `Enter::redact` cuts the token from the path of every
  `Called` event; no sign-in error holds a token; `Cookie` prints `<redacted>`.
- **The seam to `web`.** The web branch serves the page a link opens (at the operator's
  `--login-base`, default `<api origin>/v1/login`) and the form that asks for one, and calls
  `POST /v1/login`, `GET` and `POST /v1/login/{token}`, `GET` and `DELETE /v1/session`. It reads
  the session cookie through the same routes. Session listing and revocation by id exist in
  `SignIn` and have no route yet.

**Not done:** a route to list and revoke sessions; trusting a forwarded header for the source;
re-issuing the first-run link; a persistent outbox; passkey and device-key credentials; the
dashboard screens. The `ring`, `webpki-roots` and `rustls-webpki` licences are `deny.toml`
exceptions, checked on the farm. **Why:** sign-in is the one place a stolen database or a log
line becomes a stolen account, so only digests of random tokens are stored, nothing secret
prints, and the answer to "does this address exist" is one value. **Reversal:** if a hosted
adapter needs sessions in another store, `SignIn` is the only thing to reimplement behind the
same `Credentials` seam; if `GET`-spends is wanted, flip `reveals`.
