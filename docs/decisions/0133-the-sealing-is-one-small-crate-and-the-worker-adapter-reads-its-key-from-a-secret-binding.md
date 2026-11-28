---
number: "0133"
title: "The sealing is one small crate both secret adapters use, and the Worker adapter reads its key from a secret binding with a previous key for rotation"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0133 The sealing is one small crate both secret adapters use, and the Worker adapter reads its key from a secret binding with a previous key for rotation

Built (2026-11-28), A3 of `docs/hosted.md`.

- **The seal is lifted into `crates/port/secret/seal`** (`uses-ink-secret-seal`), a small crate beside the port, not
  into the port: the port stays free of a cipher and of randomness. It has `Key` (32 bytes, zeroed on drop, no
  `Clone`, redacted `Debug`, `from_hex` of exactly 64 digits) and `Seal` (`seal` and `open`, XChaCha20-Poly1305,
  the row as associated data, the same layout as before: version byte, 24-byte nonce, ciphertext, tag). It reads no
  file and logs nothing; `Sealing`, `Opening` and `Unhexed` are its typed errors, and `Opening` converts into the
  port's error. `secret-local` keeps its key file and its events and calls the seal; its tests and the layout of
  every value it ever wrote are unchanged.
- **`crates/port/secret/worker`** (`uses-ink-secret-worker`) is `Worker`: a `Seal` under the key of the secret
  binding `SEALING_KEY`, 64 hexadecimal digits. Text of another shape is `Error::Key`, which names the binding and
  the shape's fault (length, or the offset of the first character that is no digit) and never shows the text; a
  trailing newline is refused, not trimmed. The crate builds natively; only `from_env` and `rotating`
  (reading a binding) are `wasm32`.
- **Rotation holds two keys.** A sealed value carries no key id and the format stays as it is, so `Worker::previous`
  adds the old key as a second seal: opening tries the current key, then the previous one only when the current key
  refused (a malformed value is not tried again), and sealing always uses the current key. The `secret.opened`
  event says which key opened a value. The procedure is in the crate's README: put both bindings, deploy, open and
  seal every row again, delete `SEALING_KEY_PREVIOUS`. The re-seal job is control-plane work, not done here.
- **The suite got `Case`** (5 cases) like the others, with `Pair` for the case that needs a second adapter of a
  different key; `suite!` is unchanged. `Rotated` is the Worker adapter's own case, public so the rig runs it.
- **Randomness** on a Worker is `crypto.getRandomValues` through `getrandom` 0.2's `js` feature, enabled in the
  seal crate for `wasm32-unknown-unknown` only.

Numbers: native, 20 tests across seal, worker, local and conformance pass; `secret-local` passes its suite through the
shared seal. In workerd, the rig reads the key from a `[vars]` binding and passes 8 of 8 (27 to 46 ms each); the rig is
922,277 B raw and 279,658 B gzip with the four adapters.

**Not done:** a real secret (`wrangler secret put`, 3.2 of the plan), and the job that seals every row again for a rotation.
**Reversal:** the key must live outside the Worker (a KMS or a Secrets Store binding): `from_env` reads another
binding type; the seal is untouched.
