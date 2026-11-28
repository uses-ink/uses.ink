# uses-ink-authority-github

The GitHub authority, `ink:authority-github` (decision 0110): one GitHub App with the least it
needs (`docs/hosted.md` HQ13). It verifies webhook deliveries, proves that the App is installed on a
repository by the account that claimed it, makes installation tokens one call at a time, and posts
commit statuses. Nothing here reaches GitHub in tests: every call goes through a `Transport`, which has
a fake.

## What it may use

`uses-ink-authority-forge` and `uses-ink-build`. The signing key is read and used through `ring`, and
the `Signer` trait lets a Worker sign with WebCrypto instead. The full list is in
[the crate index](../../../../docs/crates.md).

## Where things are

- `crates/server/authority/github/src/hooks.rs` is `Hooks`: `X-Hub-Signature-256`, the replay window, then
  the event.
- `crates/server/authority/github/src/app.rs` is `App`: `owns`, `token` and `status`.
- `crates/server/authority/github/src/jwt.rs` and `key.rs` are the App's RS256 JWT and the PEM key reader.
- `crates/server/authority/github/tests/fixtures` holds synthetic deliveries (with signatures made by
  `openssl dgst`), API replies and a throwaway RSA key. Nothing in it is a real credential.

## How to test it

    farm cargo nextest run -p uses-ink-authority-github
