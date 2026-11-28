---
number: "0085"
title: "Recovery codes are minted once, kept as digests, and each is spent once to register a new passkey"
status: built
date: 2026-11-28
area: security
supersedes: []
---

## 0085 Recovery codes are minted once, kept as digests, and each is spent once to register a new passkey

Built (2026-11-28). It follows `docs/auth.md` section 9 (recovery codes) and closes the recovery item of
0068. Email recovery with a delay is not built.

- **A code** is 128 random bits from `Draw`, shown as 26 Crockford base32 characters and a check
  character (modulo 37, Crockford's check symbols), in groups of four. Case, dashes, spaces and the
  look-alike letters are forgiven when reading. A text that is not a code is refused as
  `auth.recovery.refused` with status 422 before the rate limit counts it and before any lookup, so a
  typo is not an attempt. `Code` has a redacted `Debug`, no `Clone`, and is zeroed when dropped.
- **At rest** is `recovery_code`: a row id, the BLAKE3 digest (context
  `uses.ink 2026-11-27 recovery code v1`, unique), the account, the time made and the time spent. No
  code text is stored or logged; the audit log gets `recovery.minted` and `recovery.used` with the row id.
  The digest read back is compared in constant time. The table is migration 203 (`control::recovery::Entry`).
- **Mint:** `POST /v1/recovery/codes` returns ten codes with 201, once. The old set is deleted in the
  same batch. It needs a session, an Origin check and a step-up (0084). `GET /v1/recovery/codes` returns
  the count left and no secret.
- **Spend:** `POST /v1/recovery` with `{code, label}` needs no session. It checks the Origin, applies a
  per-source limit (`Quota::RECOVERIES`, ten an hour, `Ceremonies::with_recoveries` sets another), then
  spends the code with a conditional `UPDATE ... WHERE used_at IS NULL` that only one caller wins. On
  success it starts a registration ceremony for that account (the same options as a signed-in add),
  and the reply drops any session cookie. `POST /v1/passkeys` then accepts the ceremony cookie alone,
  because only a spent code starts a ceremony with no session. Unknown, spent and replaced codes give one
  answer (`auth.recovery.refused`, 401).
- **No delay,** as section 9 says: the codes were made while signed in and kept offline.
- **Dashboard:** the Account screen shows how many codes are unspent and a button that makes a new set.
  The button runs in `/passkey.js` (a step-up and a JSON call), and the codes are written into the page
  once; the server-rendered page never holds them.
- **CLI:** `ink serve --data` applies `control::recovery::Entry` (203) after the passkeys' migration and takes
  `--recovery-limit` (default 10/1h) and `--step-up-window` (default 5m).
- **Not built:** the mail notice on every use (the mail crate is outside this work; the audit event is
  written).
