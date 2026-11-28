# uses-ink-secret-worker

The secret port (`uses-ink-secret`) on a Worker (decision 0133): credentials are sealed with the sealing
the self-hosted adapter shares (`uses-ink-secret-seal`, XChaCha20-Poly1305 with the row as associated
data), under a key that is a Workers secret binding.

- **The key** is 64 hexadecimal digits in the secret `SEALING_KEY` (`wrangler secret put`). Text of
  any other shape is `Error::Key`, which names the binding and what is wrong with the shape and
  never shows the text; a trailing newline is refused, not trimmed. In tests the key is a `[vars]`
  entry or a line of `.dev.vars`; `Worker::from_env` reads both the same way.
- **Rotation.** A sealed value carries no key id, so a rotation holds both keys at once:
  1. `wrangler secret put SEALING_KEY_PREVIOUS` with the old key, then `wrangler secret put SEALING_KEY` with a new one
     (`openssl rand -hex 32`), and deploy a Worker that reads both (`Worker::from_env(..).rotating(..)`).
  2. Every credential row is opened and sealed again (opening tries the current key, then the previous one, and the
     `secret.opened` event says which opened it).
  3. When no row opens with the previous key, delete `SEALING_KEY_PREVIOUS`.

  A seal that neither key opens is `Refused`, as it is without a rotation. `Rotated` is the case.
- **Randomness** is the platform's: the nonce and a test key come from `crypto.getRandomValues`, through
  `getrandom`'s `js` feature on `wasm32-unknown-unknown`.

The crate builds natively: only reading a binding is `wasm32`. The port's suite runs on it natively and in
workerd (`cargo xtask worker secret`).
