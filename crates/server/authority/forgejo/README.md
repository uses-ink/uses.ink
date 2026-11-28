# uses-ink-authority-forgejo

The Forgejo authority, `ink:authority-forgejo` (decision 0111), for Codeberg and any instance an
operator lists. It verifies Gitea-scheme webhook deliveries, proves write access to a repository by
the repo challenge, and posts commit statuses.

## What it may use

`uses-ink-authority-forge` alone. The full list is in [the crate index](../../../../docs/crates.md).

## Where things are

- `crates/server/authority/forgejo/instances.toml` is the list of instances, with Codeberg as the primary one;
  `crates/server/authority/forgejo/src/instance.rs` reads it once.
- `crates/server/authority/forgejo/src/hooks.rs` is `Hooks`: the hex HMAC in `X-Forgejo-Signature` or
  `X-Gitea-Signature`.
- `crates/server/authority/forgejo/src/forgejo.rs` is `Forgejo`: `prove` and `status`.

## How to test it

    farm cargo nextest run -p uses-ink-authority-forgejo
