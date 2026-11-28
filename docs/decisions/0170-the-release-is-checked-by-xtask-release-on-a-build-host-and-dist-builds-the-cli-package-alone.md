---
number: "0170"
title: "The release is checked by `cargo xtask release` on a build host, and `dist` builds the CLI package alone"
status: built
date: 2026-11-28
area: release
supersedes: []
---

## 0170 The release is checked by `cargo xtask release` on a build host, and `dist` builds the CLI package alone

Built (2026-11-28), the release item of M5 that needs no account. Decision 0045 chose `dist` and the
`Dockerfile`; nothing had run either of them, and the first run found three faults.

- **`dist build` linked the plugin guests natively.** With the default of building the workspace, `dist`
  ran `cargo build --workspace` for the target, which compiles the guest crates (`cdylib`s for
  `wasm32`) for `x86_64-unknown-linux-gnu` and fails in the linker (`source-git-guest`: a version script
  the system linker does not read). The release workflow would have failed the same way on every runner.
  `dist-workspace.toml` now sets `precise-builds = true`, so `dist` builds `uses-ink-cli` with `-p`.
- **The image's healthcheck asked `/`.** `ink serve` answers a request for a host no site has with a
  refusal, and `/healthz` is the route that answers `200` there (0051). The check asked the wrong path
  and would have kept a healthy container `unhealthy`. It asks `/healthz`.
- **The image had no folder to serve.** `ink serve` reads `.ink/out` of its working directory and refuses to
  start without it (`serve.folder.unservable`), also with `--data`. The image now makes `/data/.ink/out`,
  empty, which serves no site, so `--data` starts.

`cargo xtask release <step>` runs what the owner would otherwise run by hand, on a farm host, and
publishes nothing:

| Step | What it does |
|---|---|
| `plan` | `dist plan` at the pinned version: every archive, installer and checksum is listed and nothing errors |
| `build` | the components (0054), then `dist build --artifacts=local --target <this host's Linux target>`, then the built `ink` run with `--version` |
| `image` | builds the `Dockerfile` with docker, or podman with `--format docker` (an OCI image drops the healthcheck), reads the image's user and healthcheck, starts it read-only with every capability dropped and a fresh volume at `/data`, and requires the healthcheck's own request to answer |

Each step is a type (`Release`), its failures are `release::Fault` values with the command or the
container's own output as the cause, and the container, volume and image a run made are removed after
it, pass or fail. The steps are not in the gate: they need `dist` and a container runtime, which only
some hosts have, and the image step compiles `ink` a second time. `farm cargo xtask release plan` runs
where `dist` is installed (`cargo install cargo-dist --version 0.34.0 --locked`, once per host).

**Not run, so not claimed.** The targets other than Linux `x86_64` gnu, the generated workflow itself, the
installers on a clean machine, and the image on `aarch64` run only in the owner's CI and on the owner's
machines. docs/release.md lists them.

**Why:** a release path that was never run fails on the owner's first tag, with the owner's account in
front of it. **Reversal:** none needed; the steps only read and build.
