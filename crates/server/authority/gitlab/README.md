# uses-ink-authority-gitlab

The GitLab authority, `ink:authority-gitlab` (decision 0112). It verifies webhook deliveries by the
secret token header, proves write access to a project by the repo challenge, and posts commit statuses.

## What it may use

`uses-ink-authority-forge` alone. The full list is in [the crate index](../../../../docs/crates.md).

## Where things are

- `crates/server/authority/gitlab/src/hooks.rs` is `Hooks`: `X-Gitlab-Token`, the replay window, then the event.
- `crates/server/authority/gitlab/src/gitlab.rs` is `Gitlab`: `prove` (the files API on the default branch) and
  `status`.

## How to test it

    farm cargo nextest run -p uses-ink-authority-gitlab
