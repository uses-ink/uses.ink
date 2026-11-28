# uses-ink-authority-forge

What the git-forge authorities share (decisions 0110 to 0113): typed repository names, the push a
verified webhook comes to, the order every webhook is checked in, redacted secrets, the repo
challenge and the `Transport` port with a `Fake`. The three forge crates differ only in the forge's
own spelling of these.

## What it may use

`uses-ink-build` for the `Branch` and `Revision` types, and nothing else of ours: it knows no control
plane, so what it returns is a typed fact (a `Push`, a `Proven`) and the edge decides what it means.
The full list is in [the crate index](../../../../docs/crates.md).

## Where things are

- `crates/server/authority/forge/src/name.rs` is `Host`, `Segment`, `Path` and `Repo`: lower-case, bounded
  names with typed errors.
- `crates/server/authority/forge/src/hook/push.rs` is `Push`, `Ignored` (why a verified delivery queues
  nothing), `Received` and `Roster` (the repositories sites follow).
- `crates/server/authority/forge/src/hook/intake.rs` and `hook/delivery.rs` are the size limit and the replay window.
- `crates/server/authority/forge/src/secret.rs` is `Secret`: redacted `Debug`, zeroed on drop, constant-time
  compares through `ring`.
- `crates/server/authority/forge/src/call/transport.rs` is `Transport`, `Client` (stamps the configured limits)
  and `Fake` (answers from recorded replies, keeps what it was asked).
- `crates/server/authority/forge/src/call/challenge.rs` is the repo challenge and what a failed proof says.

## How to test it

    farm cargo nextest run -p uses-ink-authority-forge
