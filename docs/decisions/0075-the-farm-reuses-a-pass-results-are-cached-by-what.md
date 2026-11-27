## 0075 The farm reuses a pass: results are cached by what the job read, and only passes

Accepted, owner. A day on the build host showed 16 full `farm check` runs, stage `test` 50 times,
`sites` 38 and `wasm` 16, from reruns of a stage on a tree that had passed it, a full re-gate after
a rebase with no code change, two copies of one job started by one agent, and whole gates for a
docs-only change. `farm` now refuses to repeat work it can prove it did, so this does not depend on
agents behaving. Extends 0047.

- **A pass is keyed by everything the result depends on.** The key hashes the job name, its
  command line and variables, the digest of its inputs, the host class (the architecture, for a
  `golden` job only), the toolchain channel of `rust-toolchain.toml` and the text of the job file.
  An identical key means the same command ran on the same bytes with the same toolchain on the same
  kind of host. A hit prints `passed earlier on this tree (<id>, <time ago>)` with the original
  log and exits 0. `--fresh` skips the lookup and still records the pass.
- **The input digest is a hash of file contents, not of a revision.** Rsync compares by checksum
  but exposes no digest, and a revision id is wrong twice: a rebase changes it with no change of
  bytes, and an uncommitted edit does not change it. So the digest is over the bytes of the files
  jj tracks (jj snapshots first, so a new file counts), minus what the sync leaves out, in path
  order. A rebase onto a tree with the same bytes hits the cache; `farm fmt` and `farm bless` need
  no hook because they change files and so change the digest. A cache of file hashes under
  `target/farm/` keyed by (path, size, mtime, inode) makes a repeat digest a stat per file; a file
  written in the last two seconds is rehashed, since a later write could keep its stat.
- **Jobs may name their inputs.** `inputs = [globs]` in `.config/farm.toml` narrows the digest to
  the matching files plus `Cargo.lock`, `rust-toolchain.toml` and `.config/**`, so prose does not
  invalidate Rust jobs. The default is the whole tree, and each narrowing in this repo is argued in
  a comment: where a job might read a file the glob drops, the file stays in. A glob list is read
  in order, last match wins, `!` takes files out; a bad glob is an error with the file and line.
- **Only passes are stored.** A failure is information about a state that someone is about to
  change; replaying it would show an old failure on a fixed tree, and rerunning it is the point of
  the next call. A stored pass is replayed as truth, so a pass that is luck is never stored: a log
  with `FLAKY`, `RETRY`, `TRY n` or `TIMEOUT` lines is not kept, and a job may say `cache = false`.
  A pass is also not stored when the tree changed while the job ran (checked for a foreground job).
- **What is cached.** The jobs of `farm check` and `farm cargo` with `check`, `clippy`, `test`,
  `doc`, `deny` or `nextest run`. Never `bless`, `fmt`, `build`, `run` or `install`: they exist for
  what they leave behind.
- **Scope.** Local to the machine, in `$XDG_CACHE_HOME/uses-ink/farm/<repo>` (else `~/.cache`),
  shared by every workspace of the repo because the key holds no workspace. Entries expire by age
  and count, set in the `[cache]` table (defaults 168 hours, 256 entries).
- **Identical running jobs are one job.** A job started while one with the same workspace, key and
  inputs runs attaches to it, with `farm wait` semantics. A claim is a hard link, which is atomic,
  so two clients never both start. A job that finishes under `farm wait` or an attach records its
  pass.
- **Heavy use is visible, not refused.** `farm check` ends with a summary line, `farm status`
  counts the last 24 hours per workspace, and `--budget N` or `[budget] full_checks_per_day` warns.
  A refusal would stall a real need; a warning is counted and read.
- **A lease is the wrapper's alone.** A test server that outlived its job held a slot for 28
  minutes because it inherited the slot's lock descriptor. The command now runs with it closed and
  the wrapper kills the job's process groups when the command ends. `farm status` names each busy
  slot's holder and flags `wedged?`.

**Why:** the cheapest heavy command is the one not run, and a cache that is only a convention
needs every agent to follow it. **Reversal:** a shared remote cache would replace the local
directory; a wrong stale pass (a job that reads a file its `inputs` drop) is fixed by widening that
job's globs, or by `cache = false`.
