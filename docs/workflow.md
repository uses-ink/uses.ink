# Workflow: building and testing on the farm

Agents and people run the gate and the inner loop through `cargo xtask farm`, which sends each
job to a host with a free slot instead of compiling in the workspace. `cargo xtask check` with no
farm is unchanged: CI and a lone human still use it. The design is decision 0047; this file is
how to use it.

Every verb blocks, prints a short report and exits with a typed code. Nothing here needs a
daemon, and nothing needs `ssh` or `rsync` typed by hand: agents are refused both, and
`farm` is the only way they reach another host.

## The hosts file

The hosts live outside the repo, because the repo is public and the names of a homelab are not
part of it. The file is `$INK_FARM_CONFIG` if set, else `$XDG_CONFIG_HOME/uses-ink/farm.toml`,
else `~/.config/uses-ink/farm.toml`. With no file the farm is this machine alone. A named file
that is missing is an error; the default one that is missing is not.

```toml
[[host]]
name = "box-a"          # letters, digits, - and _; at most 32 characters
ssh = "me@box-a.lan"    # an ssh destination (your ssh config and agent apply), or "local"
slots = 1               # optional; default is cores / 6, at least 1
arch = "x86_64"         # optional; default is what the host reports (`uname -m`)
golden = true           # optional; default is true only for x86_64
mold = true             # optional; default false; link with mold
dir = "~/farm"          # optional; default ~/farm; absolute or relative to $HOME

[[host]]
name = "box-b"
ssh = "box-b"
arch = "aarch64"
mold = true

[[host]]
name = "here"
ssh = "local"           # this machine, in the workspace itself, with no copy
```

- `name` is how a host is shown and how a job id names it. `ssh` is passed to `ssh` as one
  word, so a value that starts with `-` is refused.
- A **slot** is a share of the host that one job holds from its first compile to its exit. A
  host runs as many jobs at once as it has slots, and each job gets `cores / slots` cores for
  cargo (`CARGO_BUILD_JOBS`) and for test threads (`NEXTEST_TEST_THREADS`).
- **golden** says the host may run tests and goldens whose output depends on the architecture.
  The reference architecture is x86_64 (decision 0047), so only x86_64 hosts are golden unless
  the file says otherwise.
- The arch is read on every probe, so there is nothing to cache or to go stale.
- Every host needs a POSIX login shell, `flock`, `timeout`, `tail`, `setsid`, `rsync`, `cargo`
  (rustup, in `~/.cargo/bin`) and `cargo-nextest`. The wasm stage needs the `wasm32` targets,
  `wasmtime` and `cargo-deny` as `cargo xtask check` does. A host on a copy-on-write filesystem
  (btrfs, xfs with reflinks) seeds a new workspace in seconds.

## Verbs

| Verb | What it does |
| --- | --- |
| `farm check [stage...]` | The gate, one job per stage (`lint test docs wasm sites`), all placed at once. |
| `farm cargo <args...>` | Any cargo command on a placed host, in the synced workspace. |
| `farm bless [nextest args]` | Nextest with `INSTA_UPDATE=always` on a golden host; brings back only changed `*.snap`. |
| `farm status` | Each host: arch, load, slots used, queue, and its running and queued jobs. |
| `farm workspace <name>` | A new jj workspace with a warm `target/`. `--forget <name>` removes one safely. |
| `farm wait <id>` | Blocks until a detached job ends, or the timeout passes. |
| `farm log <id>` | A job's log from its host; `--tail N` for the end. |

Flags go before the verb, or right after it; for `cargo` and `bless` everything after the first
word that is not a farm flag belongs to cargo (`--` ends the farm's flags at once).

| Flag | Meaning |
| --- | --- |
| `--detach` | Start the job and print its id; do not wait. |
| `--timeout N` | How long one call blocks, in seconds. Default 540: a tool call may take 10 minutes. |
| `--limit N` | The hard limit of the job's command, enforced on the host by `timeout`. Default 1500. |
| `--host NAME` | Use only this host. A golden job on a host that is not golden is refused. |
| `--json` | Print the machine report, schema `uses.farm/v1`. |
| `--verbose` | Print every command the farm runs, exactly. |
| `--tail N` | `log`: only the last N lines. |
| `--at REV`, `--forget` | `workspace`: start from a revision; remove instead of make. |

`farm cargo` runs on a golden host when the cargo command runs tests (`test`, `nextest`,
`xtask`); everything else (`check`, `clippy`, `build`, `doc`) runs on any host. `farm check`
puts `test` and `sites` on golden hosts and `lint`, `docs` and `wasm` anywhere.

### Exit codes

| Code | Name | Meaning |
| --- | --- | --- |
| 0 | passed | The command passed. |
| 1 | failed | The command ran and failed: a test, a lint, a compile error. |
| 2 | usage | The command line is wrong. |
| 3 | farm | The farm could not do it: bad config, no host, an unreachable host, a failed sync, a lost connection. |
| 4 | refused | The farm will not: a golden job on a host that is not golden, a workspace with uncommitted work, a path outside the workspaces. |
| 75 | running | The job is still running after the time given. Call `farm wait <id>` again; it is safe. |
| 124 | timed out | The command hit `--limit` and was stopped. |

A call that has several jobs (`farm check`) exits with the worst of them: 3 over 4 over 124 over
1 over 75 over 0.

## The inner loop

While you work, run the narrowest command that answers the question, scoped to a package:

```sh
cargo xtask farm cargo check -p uses-ink-engine
cargo xtask farm cargo clippy -p uses-ink-engine --all-targets
cargo xtask farm cargo nextest run -p uses-ink-cli -E 'test(parse)'
```

On a warm target a package-scoped check or test takes seconds. Never run the whole gate after an
edit.

The terminal gets a header (host, slot, time queued and run, the command), then a summary. For
nextest it is the counts and every `FAIL` or `TIMEOUT` test with its output, at most 60 lines
each. For cargo it is the number of errors and warnings and the first ten in short form. For a
gate stage it is the `check:` lines. The full log is streamed to `target/farm/<job>.log`.

## The finish

When a state is worth a commit, run the gate once:

```sh
cargo xtask farm check
```

The five stages run at the same time on different slots, so the wall time is the slowest stage,
not the sum. The report names each stage, its host and its time, and gives detail only for the
failures. Fix what failed, then run `farm check` again with just the stages that failed
(`farm check lint test`) and, if you want the whole gate green once more, once without arguments.
One whole `farm check` per commit-worthy state.

If a golden file changed on purpose, bless it and read every diff:

```sh
cargo xtask farm bless -p uses-ink-cli
```

Only the `*.snap` files that changed come back (never a `.snap.new`, never source), and the
list is printed. Read each diff (`jj diff <path>`) before you commit: a blessed snapshot is a
claim that the new output is right.

## Never poll

Do not write `until grep ...` loops and do not call `sleep`. A job that may take long is
started with `--detach` and waited for with one blocking call:

```sh
cargo xtask farm --detach cargo nextest run --workspace
# prints: RUNNING as box-a.18c3f9a21; continue with `cargo xtask farm wait box-a.18c3f9a21`
cargo xtask farm wait box-a.18c3f9a21
```

`wait` blocks on the host until the job ends or `--timeout` passes (default 540 s). A job that
ends returns its result and report; a job still running exits 75, and calling `wait` again is
safe. A foreground `farm` call that outlives its timeout does the same: it lets the job go on,
prints the id and exits 75.

Ctrl-c, or a dropped connection, stops a foreground job on the host and frees its slot. A
detached job, and a foreground job past its timeout, are left running on purpose.

## How many agents may run at once

At most the total number of slots of the hosts: the sum of `slots` in the file, which
`farm status` shows. More is not an error. Extra jobs wait in a fair queue, oldest first, and
the call prints `queued behind N, host X free in about Y` at most every 30 seconds. No agent
needs to coordinate with another: the slot locks on the hosts do it.

## Why the budgets are honest

The 100 ms test budgets of `.config/nextest.toml` are for a quiet machine. Six agents building
at once on one host made them fail for want of CPU, not for a bug. A slot gives each job its own
cores for as long as it runs, which is what makes a budget mean what it says. So `farm` never
widens a budget, never retries a failed test and never changes the nextest config. A test that
is over budget on the farm is slow.

## Golden hosts

Output-affecting tests and goldens run only on a golden host, which by default means x86_64.
Other architectures take the stages that do not compare output: `lint`, `docs` and `wasm`. A
golden job that cannot find a golden host fails with exit 3 and says so; one pinned with
`--host` to a host that is not golden is refused with exit 4.

## What a farm run builds

Every farm run of a host uses the same environment, so the host's `target/` stays warm:

- `CARGO_PROFILE_DEV_DEBUG=line-tables-only`: debug info with line tables only. The build and
  the link are smaller and faster, and panics and backtraces still show file and line. You lose
  local variable inspection in a debugger, which farm runs are not for.
- `CARGO_TERM_COLOR=never`, `INSTA_UPDATE=no` (`always` for `bless`), `CARGO_BUILD_JOBS` and
  `NEXTEST_TEST_THREADS` set to the cores of one slot.
- `RUSTFLAGS=-C link-arg=-fuse-ld=mold` only when the host says `mold = true` and `RUSTFLAGS`
  is not already set there.
- `CARGO_TARGET_DIR` is never set: the target is `target/` of the synced workspace.
- Incremental compilation stays on.

The farm's artifacts are not a plain `cargo build`'s: the debug setting changes the profile, so
a `cargo build` in the same `target/` rebuilds everything once. On the host called `local` this
applies to your own `target/` too, because `local` runs in place. Use one or the other in a
workspace.

## Syncing

The first job of a workspace on a host seeds its directory (`<dir>/<repo>/<workspace>`) from the
newest sibling there by a reflink copy, so third-party crates are warm. Then `rsync -a --delete`
makes it equal to the workspace, mtimes kept, so cargo rebuilds only the crates that changed.
Excluded: what `.gitignore` ignores, `.jj`, `.git`, `.impeccable`, `target`, and `.env`,
`.env.*`, `*.pem` and `*.key`. The rsync target is always under the host's `dir`. A workspace is
one agent's: do not edit files while a job of that workspace is running unless you want the job
to see them.

State on a host is under `<dir>`: `slots/`, `queue/`, `jobs/<id>/`, `locks/`. `farm status`
removes job state older than seven days and says how much.

## New workspaces

Use `farm workspace` instead of `jj workspace add`:

```sh
cargo xtask farm workspace parser           # from main
cargo xtask farm workspace parser --at <rev>
cargo xtask farm workspace --forget parser
```

It adds `../uses.ink-ws/<name>` and seeds its `target/` with `cp -a --reflink=always` from the
newest warm sibling, so the workspace builds in seconds instead of ten minutes: the workspace's
crates rebuild, the third-party crates do not. Where the filesystem has no reflinks it says so
and copies in full. `--forget` refuses unless the path is a direct child of `uses.ink-ws`, is a
jj workspace of this repo, is not the one you are in and has no uncommitted changes.

## Briefing an agent

Paste this into an agent's prompt:

> Build, lint and test only through `cargo xtask farm`, never with plain `cargo` and never with
> `ssh` or `rsync`. While working, run package-scoped commands: `cargo xtask farm cargo check -p
> <crate>`, `cargo xtask farm cargo clippy -p <crate>`, `cargo xtask farm cargo nextest run -p
> <crate> -E 'test(<name>)'`. Do not run the whole gate per edit. When a state is ready to
> commit, run `cargo xtask farm check` once and fix what it reports; if a golden file changed on
> purpose, run `cargo xtask farm bless` and read every diff it lists. Never poll and never write
> sleep loops: start long jobs with `--detach` and block with `cargo xtask farm wait <id>`; exit
> code 75 means still running, call `wait` again. If the queue says you are waiting, wait: the
> slots are what keep the 100 ms test budgets honest, so do not widen a budget or retry a test
> to get past a failure. Make a workspace with `cargo xtask farm workspace <name>`, not `jj
> workspace add`. Exit codes: 0 passed, 1 failed, 2 usage, 3 farm trouble, 4 refused, 75 still
> running, 124 hit its limit.
