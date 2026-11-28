---
number: "0130"
title: "The API crates build for wasm32 because sockets and files are features of the crates that use them"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0130 The API crates build for wasm32 because sockets and files are features of the crates that use them

Built (2026-11-28), W2's prerequisite of `docs/hosted.md` and the open finding of 0096: `uses-ink-edge-api` did
not build for `wasm32-unknown-unknown`, because tokio's `net` feature reached it and mio refuses the target.

- **The workspace's tokio has no `net`, `fs` or `signal`.** `[workspace.dependencies] tokio` is `io-util`, `macros`,
  `rt`, `sync` and `time`. A crate that opens sockets, files or the Ctrl-C handler adds the feature in its own
  manifest: `uses-ink-cli` (`net`, `signal`), `uses-ink-edge-axum` (`net`). `time` stays, so `uses-ink-clock` builds
  unchanged.
- **Two crates split by a feature, not by a new crate.** `uses-ink-edge-folder` has `files` (the `Deploys` and
  `Files` readers, which need tokio's `fs`); without it the crate is the manifest checks the intake uses
  (`Unchecked`, `Checked`, `MANIFEST`, `Error`). `uses-ink-authority-dns` has `system` (the resolver that asks
  nameservers over UDP and TCP); without it `Chosen` is `Offline` and the `Resolver` port, `Fake` and `Dns` are
  whole. Both features are on by default for the crate itself, so `cargo nextest -p` of either is unchanged. The
  workspace entries say `default-features = false`, so a dependent asks for the feature by name: the CLI and
  `uses-ink-edge-control` take `files`, the CLI takes `system`. Native behaviour is identical.
- **Randomness on a Worker is the platform's.** `uses-ink-clock`'s `System` draws through `rand_core` 0.6, which
  reads `getrandom` 0.2; on `wasm32-unknown-unknown` that needs its `js` feature (`crypto.getRandomValues`). The
  API crate enables it for that target only, beside the `getrandom` 0.4 line its passkey verifier needs.
- **The gate checks it.** `uses-ink-edge-api` joins the `wasm32-unknown-unknown` clippy list of the `lint` stage,
  so a socket or a file reaching the API again fails `farm check lint`.

**Not done:** `webauthn-rs-core`'s clock patch (0096) is still a `[patch]` of `spikes/hosted`, so a registration
runs in the spike's Workers only; it moves to the workspace with the Worker crate (W2 of the plan).
**Reversal:** a crate other than these two needs sockets on a Worker: split it the same way.
