## 0047 Builds and tests run on a farm of slots, placed by the `farm` binary

Accepted, owner. Agents and people run the gate and the inner loop through `farm`
(docs/workflow.md), which places each job on a host of the user's config and runs it there in a
slot. `cargo xtask check` is unchanged for CI and humans.

- **A binary of its own, installed once.** `farm` is the crate `farm` at `tools/farm`, not an xtask
  task. `cargo xtask` is `cargo run -p xtask`, so every workspace and every CI job compiles
  xtask, and a farm inside it would slow CI and every task call. As a member of its own it depends
  on std, serde, serde_json and toml only (nothing of xtask or the engine, no async runtime, no
  clap) and is installed with `cargo install --path tools/farm --locked` into `~/.cargo/bin`:
  precompiled once, called by every workspace and agent as `farm ...`, which is also the one
  shape a permission rule `Bash(farm *)` has to match. `farm --version` prints the hash of the
  source it was built from (`build.rs`), and `farm status` warns when the checkout's `tools/farm`
  hashes differently, so a stale install is said and not silent.
- **It knows no project.** The jobs `farm check` runs are the repo's `.config/farm.toml`
  (`[job.<name>]` with `run`, `golden`, optional `limit`). uses.ink declares `lint`, `test`,
  `docs`, `wasm` and `sites`, with `test` and `sites` golden; the farm reads no `Stage`.

- **Slots through `flock`, no daemon.** Each host has `slots/<n>.lock` files. A job holds one
  with `flock` for as long as its command runs, so a slot is free exactly when nothing runs in
  it, and a dead job frees it. A waiting job holds a ticket file in `queue/` with its pid; the
  oldest live ticket takes the next free slot and a ticket of a dead pid is removed by whoever
  finds it. Any number of agents and sessions coordinate through the host's filesystem with no
  process of ours running between jobs. Each slot gets `cores / slots` cores for cargo and for
  nextest's test threads.
- **Hosts live outside the repo**, in `~/.config/uses-ink/farm.toml` or `$INK_FARM_CONFIG`: the
  repo is public and the names of a homelab are not part of it. No file means this machine
  alone, so the farm is always usable.
- **Reflink seeding.** A workspace's `target/` is seeded by `cp -a --reflink=always` from the
  newest warm sibling, on this machine (`farm workspace`) and on a host (the first sync), so a
  new workspace compiles its own crates and not the 800 third-party ones. Without reflinks it
  copies in full and says so.
- **Golden hosts.** Output-affecting tests and goldens run only on a golden host, by default
  x86_64, the architecture the goldens are blessed on. Other architectures take the jobs the file
  does not mark `golden`. `bless` refuses a host that is not golden.
- **The gate is fanned out by job.** `farm check` places every job of the file at once (here the
  stages CI already runs as parallel jobs), so the wall time is the slowest job and not the sum.
  What a stage runs did not change.
- **`farm wait` replaces polling.** Across 14 agents, 182 minutes went to hand-written `until
  grep` loops and 34% of wall time to calls of 20 s or more. A long job starts with `--detach`
  and one blocking `wait` (or a foreground call that passes its 540 s timeout) returns the
  result, or exit 75 and "call again". The host does the waiting, in one call.
- **A job's lifetime is its client's, unless it was let go.** A foreground job is tied to its
  connection: ctrl-c or a dropped connection stops it on the host and frees the slot. A detached
  job, or one past its timeout, is let go on purpose and ends by itself, or at `--limit`.
- **Jobs run the `ci` nextest profile.** The 100 ms budgets are for the owner's quiet machine;
  the farm's hosts are shared and a pure-logic test took 118 ms beside four agents. The farm sets
  `NEXTEST_PROFILE=ci` unless the caller chose a profile; its 5 s cap still stops a hang. The
  farm never retries a test and never changes `.config/nextest.toml`.
- **Everything is argv.** A host's program is fixed text with positional parameters; the one
  shell line, ssh's remote command, is made of `Quoted` words. Nothing from the config or the
  arguments is shell syntax. Hosts are reached with the user's own `ssh` and `rsync`, so
  credentials and the agent stay the user's.

**Why:** six agents compiling at once drove one machine to load 61 on 14 cores, 35 of 111 gate
runs failed or timed out mostly on the 100 ms budgets from CPU contention, and every workspace
compiled all of its crates cold. **Reversal:** a hosted CI-style queue, or a build cache shared
between workspaces, would replace the slot files; the verbs and the exit codes would stay.
