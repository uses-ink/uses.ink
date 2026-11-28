# uses-ink-secret-seal

The sealing the secret adapters share (decision 0133): a `Key` of 32 bytes, and a `Seal` that seals a
credential to the row that holds it with XChaCha20-Poly1305 under a fresh 24-byte nonce, and opens it.

Layout of a sealed value: one version byte (`1`), the 24-byte nonce, the ciphertext and its 16-byte tag.
The adapters bring the key (a file for `uses-ink-secret-local`, a Workers secret for
`uses-ink-secret-worker`) and say what happened in their own events; this crate reads no file and logs nothing.
A `Key` has no `Clone`, a redacted `Debug` and is zeroed when dropped; `Key::from_hex` takes exactly 64
hexadecimal digits.
