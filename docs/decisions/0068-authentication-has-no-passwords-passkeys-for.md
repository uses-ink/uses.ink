---
number: "0068"
title: "Authentication has no passwords: passkeys for people, signed requests for machines"
status: accepted
date: 2026-11-27
area: security
supersedes: []
---

## 0068 Authentication has no passwords: passkeys for people, signed requests for machines

Accepted (2026-11-27). The owner chose the direction ("no passwords anywhere"), reviewed the
design in `docs/auth.md` and `docs/auth-preview.html`, and took the recommendation on every open
choice of its section 23. Passkeys are not built yet; the sign-in of 0066 is the bootstrap. In short:

- **People** sign in with **passkeys** (WebAuthn, discoverable, user verification required, ES256,
  attestation none), on `webauthn-rs-core` behind our own `Passkeys` port, after a spike shows it
  builds for `wasm32-unknown-unknown`. After sign-in a **session**: a 256-bit id of which only a
  context-separated BLAKE3 digest is stored, in a `__Host-` `HttpOnly` `Secure` `SameSite=Lax`
  cookie, with absolute and idle limits, a list and revocation. CSRF is Lax plus an Origin and
  Fetch Metadata check and no state change on GET. The API host reads no cookie and the dashboard
  host accepts no signature.
- **The CLI and CI** sign every request with an **Ed25519** key, in a fixed one-header profile
  (`Ink-Sig-1`) modelled on RFC 9421 but not wire compatible with it: method, canonical target,
  BLAKE3 body digest, audience, time, nonce and key id under a context line, no algorithm field, a
  strict parser, errors that leak nothing. Devices enrol with a device flow (RFC 8628) whose secret
  is the key pair, the CLI showing a key fingerprint and the dashboard approving with a passkey.
  Deploy keys are scoped to one site, expire (90 days by default) and rotate with overlap. A
  device key can revoke but never mint a credential.
- **Recovery** is a passkey, a recovery code (128 random bits, digests only), or an email link that
  starts a 48-hour recovery with notices and cancel. There is no magic-link login. Self-host's first
  run prints a one-time link; `--admin-link` re-issues it for the folder's operator.
- **The owner token** (0052) is deleted. The operator credential is `<data>/operator-keys`, a file
  of public keys, enrolled with `ink login --data`.
- **Rights** are a typed `Right` enum on a `Scope` (`Account`, `Site`), least privilege by default,
  composed by union with keys as attenuations; orgs and roles arrive with M4.
- **Dependencies** need licence exceptions in `deny.toml`: the `webauthn-rs` tree (MPL-2.0) and
  `ed25519-dalek` with `curve25519-dalek` (BSD-3-Clause); `ed25519-compact` (MIT) is the alternative.

**Why:** a database leak, a log, a proxy or a phished form yields nothing that signs in, and no
person ever holds a secret they chose. **Reversal:** the wasm spike fails and our own verifier is
too large, or the licence exceptions are refused: then the passkey library or the Ed25519 crate
changes behind its port, and the scheme stands. The open choices are the last section of
`docs/auth.md`.

**Build note (slice 1, 2026-11-27).** The signed-request half is built; passkeys and the dashboard
are not. What differs from the text above, or was left out:

- The profile, the verifier, the key table, device-code enrolment and the `auth_event` audit
  table are built (`sig`, `crates/server/control/keys`, `crates/server/edge/api`, `ink login|logout|whoami|key`). The verifier
  runs in the documented order; the nonce store is in memory, bounded per key, with a boot barrier
  of one skew window so a restart cannot replay.
- `ink serve --data` makes no key: the server holds no private key. It makes an empty
  `<data>/operator-keys` and prints `ink login --data <folder>`, which makes the pair on the
  client and appends the public line. A removed line is a refused signature (`auth.signature.refused`);
  `auth.key.revoked` is for keys in the table. An `owner-token` file of an older version is left
  in place, unread, and the start says so (`auth.token.retired`).
- The operator key is not an approver: only a signed-in person at the Door approves an enrolment.
  The Door stays mounted beside the API until the dashboard host exists.
- The private key lives in a 0600 file under the config folder, not a keyring. `ink key add`
  prints a key only to a pipe or `--out`, never to a terminal.
- Not built: rotation with overlap (`--replaces`), passkeys, and a fuzz target (the repository
  has no fuzz infrastructure; the strict parser is covered by property tests with mutation). The
  `ed25519-dalek` and `curve25519-dalek` licence exceptions of this decision are in `deny.toml`.

**Build note (slice 2, passkeys, 2026-11-27).** Passkeys and their dashboard routes are built.

- **Spike: it builds.** `webauthn-rs-core` 0.6.2 compiles and links for `wasm32-unknown-unknown`
  (`spikes/passkeys`), with `getrandom` 0.4 `wasm_js` and `uuid` `js`, both set in
  `crates/server/edge/api/Cargo.toml` for that target. Not run in `workerd`, size not measured, and no browser was
  involved: the verifier is tested natively. The tree is 167 crates; the workspace pins
  `=0.6.2` because its constructor is `new_unsafe_experts_only` and its API may move in a minor.
  `rsa` resolves to 0.10.0, past RUSTSEC-2023-0071.
- **Licences.** Exactly four crates are MPL-2.0: `webauthn-rs-core`, `webauthn-rs-proto`,
  `webauthn-attestation-ca`, `crypto-glue` (section 15 of `auth.md` named one more that is not a
  crate in 0.6.2). **Decision: crate-scoped exceptions in `deny.toml`, accepted.** MPL-2.0 is
  file-level: it binds changes to its own files, and we link it unmodified. It is compatible with
  the AGPL of this repository, and scoping the exceptions to the four names makes a fifth MPL crate
  a new decision. The other new crates are MIT or Apache-2.0.
- **Where it lives.** The `Passkeys` port, the typed ceremonies (`Creation`, `Asking`, `Created`,
  `Asserted`), the refusals (`Reason`, `Refusal`), the audit events and the store (`Registry`) are
  in `control::passkeys`. The `webauthn-rs-core` adapter (`Webauthn`) and the routes
  (`Ceremonies`) are in `crates/server/edge/api`; `web` has the Account list, the sign-in page and the one script
  `passkey.js`, which is static text. The relying party comes from `--rp-id` and `--rp-origin`
  (default the API host and its origin), and `ink serve` prints it. The store keeps the first
  passkey's id; a changed id refuses with `auth.relying-party.changed` and names how many would stop.
- **Ceremonies** are single use: the cookie secret's digest is the row, spent by a conditional
  delete before anything is verified, five minutes, and bound to the flow (register or sign in) and
  the account. A passkey session is the same session of 0068 (digest stored, 30 d absolute, 7 d
  idle) and never signs an API request: the API host reads no cookie.
- **Counter.** The library is handed counter 0 and `Counter::judge` decides: a hardware passkey
  whose counter does not rise once it was nonzero is refused, a synced one is flagged and audited.
  A change of the backup-eligible flag is refused both ways.
- **Deviations.** Routes are `/v1/passkeys...` in `crates/server/edge/api` (JSON) rather than `/auth/...` in
  `web`; the Origin and Fetch Metadata check is on the new routes only, not yet on the older Door
  routes; the stored key is the JSON of the library's COSE key; the credential link to a session is
  a `passkey_session` table, because `db` has no ALTER. Operator registration goes through the
  admin-link session.
- **Not done.** The delayed email recovery and the browser matrix (spike 2) stay open. The run in
  `workerd` is done (0096): sign-in runs as is; registration needs a clock patch. Built since (2026-11-28): the challenge limit and the Origin check (0083), step-up (0084) and
  recovery codes (0085).
- **Tests.** `crates/server/edge/api/tests/passkey.rs` drives the real handlers with a software authenticator
  (`tests/support/authenticator.rs`, p256 ECDSA): the full ceremony, wrong origin, wrong rp id,
  replayed and expired challenge, counter rollback, unknown credential, user verification missing.

**Follow-up (2026-11-28, 0152).** The gate of section 8 is one switch for every caller, and that is accepted. The ask is
limited per source (`--enrol-limit`), so one address cannot fill the 20 waiting keys, but about four can; and a
signed-in person, which anyone can become, can close approval for a minute with five wrong codes, again each minute. A
per-source cap on waiting keys needs the source stored on the key row, and the table cannot gain a column. The
operator's tools are `--enrol-limit` and the gate's own numbers. The signature window's edge is closed too: a nonce is
kept through the last second its request passes the skew check, so a request replayed at exactly the window is a
replay (`auth.signature.replayed`).
