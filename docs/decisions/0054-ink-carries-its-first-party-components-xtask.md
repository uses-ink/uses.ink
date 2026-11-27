## 0054 `ink` carries its first-party components; xtask lists them, the build script embeds them

Built (2026-11-27), closing M2's "bundle the components into `ink`". Supersedes the bullet of 0041
that reads components from a folder.

- **One list.** `cargo xtask component` (the `Guest` list of `xtask/src/build/component.rs`) builds every
  guest and writes `target/plugin/<name>.wasm` plus `target/plugin/bundle.json`
  (`uses.bundle/v1`): for each guest that `ink` carries, its plugin id, file, version and BLAKE3.
  `ink:format-markdown`, `ink:format-obsidian`, `ink:theme-quiet` and `ink:source-git` are carried;
  the test guest `hostile` is not. `cli/build.rs` reads `bundle.json`, checks every file against
  its hash and writes the registry (`Bundled`, a const slice of `Entry`: id, version, hash,
  `include_bytes!`) that `cli/src/bundle.rs` includes. No component is named in `cli/`: a guest
  added to xtask's list with an id is carried with no other edit.
- **The prerequisite is typed, and depends on the profile.** A release-profile build (`cargo
  install --path cli`, `dist`, the image) with no `bundle.json` fails with the command to run
  (`ink carries its components and <dir> holds none: run cargo xtask component first`), and a
  file that does not match its hash fails the same way. A debug build with none carries nothing:
  `cargo check`, clippy and the docs stage need no wasm build, and asking such an `ink` for a
  component is `Error::Unbundled` (`component.bundled.missing`) that says to run the command and
  build again. `INK_COMPONENTS` points the script at another folder. The script reruns when the
  list or a file changes, and, when there is no list yet, when the folder appears.
- **A folder can override, one component at a time, and `ink` says so.** `ink build --check` no
  longer needs `--components`. With it, a file of that folder (`<name>.wasm`, the part of the id
  after the colon) replaces the bundled component of that name; a file it lacks is the bundled
  one. Every component a run uses is a notice, `plugin.component.used` (`-v`: plugin, BLAKE3,
  `bundled` or the file), and `components` in `--json` lists id, version (unknown for a folder's
  file), hash and origin. The compiled `.cwasm` cache moved to `components/` of `--cache`, since
  it is keyed by hash and shared safely.
- **Cache keys.** A theme's `identity` is the BLAKE3 of the component's bytes and `Cargo.lock`,
  so a changed bundled theme changes every page key, and the same bytes from a folder give the
  same keys as bundled. A format has no identity: what it makes (the document) is what pages are
  keyed by, so a changed format changes the keys of the pages whose documents changed, and the
  notice carries its hash for the rest.
- **Wiring.** The Dockerfile adds the wasm target and runs `cargo --locked xtask component`
  before `cargo build`, in one step, since the cache mount holds `target/`. `dist-workspace.toml`
  sets `github-build-setup = "../build-setup.yml"`, whose one step runs `cargo --locked xtask
  component` in every build job (`release.yml` carries the same step, edited by hand like the
  checkout version, since dist 0.34 is not installed here to regenerate it). The `Release` rule
  gains two invariants: the Dockerfile runs `xtask component` before `cargo build`, and
  `release.yml` runs it. `cargo xtask check` already builds the components before the tests and
  before `sites`; `sites` now runs `ink build --check` on the bundled ones, which also tests the
  embedding.
- **Reproducibility.** The same remap flags of the Dockerfile reach the wasm build, and
  `bundle.json` records every hash, so two builds' components are compared by one file. Not
  verified: that a release workflow's runners (different hosts) write byte-identical wasm.

Measured (dist profile, `x86_64` Linux, `cli/tests/measure.rs`): `ink` is 56,590,936 bytes without
the components and 60,144,096 with them, +3,553,160 (+6.3%), of which 3,550,196 are the four
components. `ink --version` takes a median of 2.4 ms bundled and 2.8 ms unbundled, and `ink check`
of the blog fixture 3.5 ms and 3.7 ms: the embedded bytes are mapped, not read, and no difference
shows. A component is compiled on first use, as measured in 0041 (a cached load 5 to 10 ms).

**Why:** one archive or image that works with no side folder, a registry that cannot disagree with
what xtask built, and a prerequisite that fails with its own fix instead of a missing-file
error later. **Reversal:** precompiling the components at build time (`.cwasm` embedded, as the
cache stores them) would save the first compile but ties the binary to a wasmtime and CPU; it
waits for a measured first-run cost that matters.
