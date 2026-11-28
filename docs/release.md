# Release

How `ink` is released. Nothing here runs by itself except the tag-triggered workflow; every step
that publishes is the owner's. Decision 0045 says why it is built this way.

## What a release is

A git tag `vX.Y.Z` on `main`. The tag starts `.github/workflows/release.yml`, which `dist`
(formerly cargo-dist, config in `dist-workspace.toml`) generated. It produces, on a GitHub
Release:

| Artifact | Targets |
|---|---|
| `uses-ink-cli-<target>.tar.xz` (`.zip` on Windows) holding `ink` and the three licence files | `x86_64` and `aarch64` Linux `gnu` and `musl`, `x86_64` and `aarch64` macOS, `x86_64` Windows (msvc) |
| `uses-ink-cli-installer.sh`, `uses-ink-cli-installer.ps1` | all of the above |
| `<archive>.sha256`, `sha256.sum`, `source.tar.gz` and its checksum | |

The archive and installer names follow the package name, `uses-ink-cli`; the binary is `ink`.

The container image is not part of the workflow. It is built from the `Dockerfile` by the owner.

## The components

`ink` carries its first-party components inside the binary (decision 0054), built by
`cargo xtask component` into `target/plugin/` along with a `bundle.json` that `crates/surface/cli/build.rs`
embeds. A release-profile build without them fails and names that command, so the three release
paths run it first:

- `cargo install --path cli --locked`: run `cargo xtask component` in the checkout beforehand.
- `dist`: `github-build-setup` in `dist-workspace.toml` points at `.github/build-setup.yml`, whose
  step runs it in every build job; `release.yml` holds the same step.
- The image: the `Dockerfile` adds the wasm target and runs it before `cargo build`, in one step.

A debug `cargo build -p uses-ink-cli` from a clean checkout works without them and carries none;
`ink` then reports `component.bundled.missing` when asked for one. `INK_COMPONENTS` names another
folder for the build script.

## Crates

Every crate has `publish = false` (workspace manifest). Nothing goes to crates.io; `ink` is
installed from the archives. `dist` ignores unpublished binaries, so the CLI crate sets
`dist = true` under `[package.metadata.dist]`.

`dist-workspace.toml` sets `precise-builds = true`: without it `dist` builds the whole workspace for each
target, which links the plugin guests (`cdylib`s for `wasm32`) natively and fails.

## Changelog

There is no changelog file. `dist` builds release notes from `CHANGELOG.md` or `RELEASES.md` when
one exists, and otherwise leaves the notes to GitHub. The source is the commit history (one-line
conventional commits): edit the release notes on GitHub from `jj log -r 'vPREVIOUS..main'`. The
owner decides whether to add a file.

## Steps

1. Bump `version` in `[workspace.package]` in `Cargo.toml`; run `cargo --locked check` so
   `Cargo.lock` follows. Commit `chore: release X.Y.Z`.
2. `cargo --locked xtask check` passes (its layout step runs the release rule, and `sites` builds
   every fixture through the bundled components).
3. `cargo xtask release plan` (needs `dist` 0.34.0: `cargo install cargo-dist --version 0.34.0 --locked`)
   lists the artifacts above and no error; `cargo xtask release build` builds this host's archive and
   runs its `ink`; `cargo xtask release image` builds and probes the image. Agents run these through
   the farm, `farm cargo xtask release plan`.
4. Owner: push `main`, then the tag (create `vX.Y.Z`, push it). The workflow runs.
5. Owner: when it is green, check the Release page, run the shell installer on a clean machine,
   and edit the notes.
6. Owner: the image.
   ```
   docker build -t ghcr.io/uses-ink/uses-ink:X.Y.Z -t ghcr.io/uses-ink/uses-ink:latest .
   docker login ghcr.io
   docker push ghcr.io/uses-ink/uses-ink --all-tags
   ```
   The registry login and the package's visibility are the owner's. By default the image runs
   `ink serve --addr 0.0.0.0:8080`, which serves the empty `/data/.ink/out` and builds no site: the guide
   (`docs/selfhost.md`) gives the command that turns the API on, `--data /data/state`.

## Hand edits to `release.yml`

`allow-dirty = ["ci"]` lets `dist generate` leave the file as edited, and `dist generate` overwrites
them, so a `dist` upgrade (change `cargo-dist-version`, run `dist generate`, run `dist plan`)
applies each edit again. Each one has a check in the layout rule:

| Edit | Why | Rule |
|---|---|---|
| every `uses:` pinned to a full commit hash, the tag in a comment (checkout is also `v7`, newer than dist emits) | a moved tag cannot change what runs | `Unpinned` |
| top-level `permissions: contents: read`, `contents: write` on `host` only | a build script in a dependency runs in the build jobs | `WidePermissions` |
| tag patterns `v[0-9]+.[0-9]+.[0-9]+` and the same with `-[0-9A-Za-z.]+`, no `*` | a tag name holds only shell-safe characters, so `plan` and `dist ${{ ... }}` lines cannot be injected | `Glob` |
| `plan` fetches full history and runs `git merge-base --is-ancestor "$GITHUB_SHA" origin/main` | every job needs `plan`, so a tag off `main` publishes nothing | `Ancestry` |
| `gh release create "$RELEASE_TAG"`, the tag passed through `env:` | no expression in a script | `Interpolated` |
| a step running `cargo xtask component` (from `.github/build-setup.yml`) | decision 0054 | `WorkflowComponents` |

`ci.yml` has no tag trigger and no write permission: the same rules keep its actions and tools
pinned (`tool: name@X.Y.Z`) and its `permissions:` read-only. To raise a pin, resolve the tag with
`curl https://api.github.com/repos/OWNER/REPO/git/ref/tags/TAG` (and the tag object, when its type
is `tag`) and write the commit hash. The check that `ci.yml` passed on the tagged commit is the
owner's (step 4); the workflow checks only that the commit is on `main`.

## What the rule checks

`cargo xtask layout` fails when: no targets are listed; no crate, or more than one, sets
`dist = true`; `release.yml` was not generated by the pinned version or runs on anything but tags;
a workflow has an action not pinned to a commit hash, an unversioned `tool:`, or no read-only
top-level `permissions:`; `release.yml` has the hand edits above missing; `.dockerignore` is not an
allowlist that covers every workspace member; the Dockerfile's rust tag differs from `rust-toolchain.toml`, its `--package` or `ENTRYPOINT` is not
the released crate and binary, or its `CMD` is not a command `ink` has, or `serve` without `--addr 0.0.0.0:<port>`; the Dockerfile does not run
`xtask component` before `cargo build`, or `release.yml` never runs it. A command is allowed
while it is on the rule's `PENDING` list (empty: `ink serve` exists).

## Known gaps

What `cargo xtask release` has run, on a farm host (decision 0170), and what has not run.

- **Run:** `dist plan` at 0.34.0 (`release plan`) lists the seven archives, both installers and every
  checksum with no error; `dist build --artifacts=local` for Linux `x86_64` gnu (`release build`) writes
  the archive and its checksum, and the `ink` in it prints its version; the image builds from the
  `Dockerfile` on musl, runs read-only as user 65532 with no capability and answers `/healthz`
  (`release image`, on podman). This found that `dist` built the plugin guests natively
  (fixed by `precise-builds`), that the healthcheck asked the wrong path, and that the image had no
  `.ink/out`, so `ink serve --data` would not start.
- **Not run:** the generated workflow (no workflow can run outside GitHub), including the tag-on-main
  step, the per-job permissions and the pinned actions; the macOS, Windows and `aarch64` targets and the
  musl archives; the installers on a clean machine; whether runners write byte-identical wasm.
  `dist` may need `GH_TOKEN` in a build job (it stays, now read-only there).
- **Not set up:** signing. The archives have checksums and no signature, and the installers verify
  the checksum only. A key, or GitHub's build attestations, is the owner's choice (decision 0170).
- `RUSTSEC-2024-0436` is ignored by id (cargo-deny cannot scope it); `deny.toml` says how to check
  that only `rav1e` uses `paste`.
