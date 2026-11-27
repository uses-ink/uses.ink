# Workflow: building and testing on the farm

Agents and people run the gate and the inner loop through `farm`, which sends each
job to a host with a free slot instead of compiling in the workspace. `cargo xtask check` with no
farm is unchanged: CI and a lone human still use it. The design is decision 0047; this file is
how to use it.

`farm` is its own binary (`tools/farm`), not an xtask task, so building xtask or running CI never
compiles it. It knows no project: what `farm check` runs is declared by the repo, below.

## Install

Once per machine, and again when `farm status` says the installed one is behind:

```sh
cargo install --path tools/farm --locked
```

It lands in `~/.cargo/bin/farm`, built in release mode, so every workspace and every agent calls
the same precompiled binary. `farm --version` prints the version and the hash of the source it
was built from (`farm 0.0.0 (source 1f3a...)`). `farm status` hashes `tools/farm` of the
workspace it runs in and warns, with the install command, when the two differ.

Every verb blocks, prints a short report and exits with a typed code. Nothing here needs a
daemon, and nothing needs `ssh` or `rsync` typed by hand: agents are refused both, and
`farm` is the only way they reach another host.

## The jobs file

The repo says what `farm check` runs, in `.config/farm.toml`, committed:

```toml
[job.lint]
run = "cargo xtask check lint"   # split on white space and run as a program: no shell reads it
golden = false                   # required; true for a job whose output depends on the architecture

[job.test]
run = "cargo xtask check test"
golden = true
limit = 900                      # optional: this job's hard limit in seconds
```

An optional `[workspace]` table lists names that `farm workspace --forget` may delete although jj
does not track them (see New workspaces):

```toml
[workspace]
disposable = [".ink"]            # file or directory names, matched at any depth
```

A disposable name is letters, digits, `-`, `_` and `.`, at most 64 characters, with no slash, and
not only dots; any other key under `[workspace]` is an error. This repo declares `.ink`, the
fixture builds' cache.

Jobs run in the order of the file. A job name is lowercase letters, digits, `-` and `_`, at most
32 characters. `farm check` with no names runs every job at once; `farm check lint test` runs
those, in that order. This repo declares `lint`, `test`, `docs`, `wasm` and `sites`, the stages
of `cargo xtask check <stage>`; `test` and `sites` are golden. A file that is missing, does not
parse or names an unknown job is an error (an unknown job is exit 2).

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
| `farm check [job...]` | The repo's jobs from `.config/farm.toml` (here `lint test docs wasm sites`), all placed at once. |
| `farm cargo <args...>` | Any cargo command on a placed host, in the synced workspace. |
| `farm bless [nextest args]` | Nextest with `INSTA_UPDATE=always` on a golden host; brings back only changed `*.snap`. |
| `farm fmt` | `cargo fmt --all` on a placed host; brings back only the `.rs` files whose bytes changed. |
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
| `--limit N` | The hard limit of the job's command, enforced on the host by `timeout`. Default: the job's own `limit`, else 1500. |
| `--host NAME` | Use only this host. A golden job on a host that is not golden is refused. |
| `--json` | Print the machine report, schema `uses.farm/v1`. |
| `--verbose` | Print every command the farm runs, exactly. |
| `--version` | Print the version and the source hash the binary was built from. |
| `--tail N` | `log`: only the last N lines. |
| `--at REV`, `--forget` | `workspace`: start from a revision; remove instead of make. |

`farm cargo` runs on a golden host when the cargo command runs tests (`test`, `nextest`,
`xtask`, `insta`); everything else (`check`, `clippy`, `build`, `doc`) runs on any host. `farm check`
puts the jobs the file marks `golden` (here `test` and `sites`) on golden hosts and the rest
anywhere.

### Exit codes

| Code | Name | Meaning |
| --- | --- | --- |
| 0 | passed | The command passed. |
| 1 | failed | The command ran and failed: a test, a lint, a compile error. |
| 2 | usage | The command line is wrong. |
| 3 | farm | The farm could not do it: bad config, no host, an unreachable host, a failed sync, a lost connection. |
| 4 | refused | The farm will not: a golden job on a host that is not golden, a workspace with uncommitted or untracked work, a path outside the workspaces, a `farm fmt` that would overwrite a file you edited after it started. |
| 75 | running | The job is still running after the time given. Call `farm wait <id>` again; it is safe. |
| 124 | timed out | The command hit `--limit` and was stopped. |

A call that has several jobs (`farm check`) exits with the worst of them: 3 over 2 over 4 over
124 over 1 over 75 over 0.

## The inner loop

While you work, run the narrowest command that answers the question, scoped to a package:

```sh
farm cargo check -p uses-ink-engine
farm cargo clippy -p uses-ink-engine --all-targets
farm cargo nextest run -p uses-ink-cli -E 'test(parse)'
```

On a warm target a package-scoped check or test takes seconds. Never run the whole gate after an
edit.

The terminal gets a header (host, slot, time queued and run, the command), then a summary. For
nextest it is the counts and every `FAIL` or `TIMEOUT` test with its output, at most 60 lines
each. For cargo it is the number of errors and warnings and the first ten in short form. For a
job that prints `check:` lines it is those lines. The full log is streamed to `target/farm/<host>.<token>.log`.

## The finish

When a state is worth a commit, run the gate once:

```sh
farm check
```

The jobs run at the same time on different slots, so the wall time is the slowest job, not the
sum. The report names each job, its host and its time, and gives detail only for the failures.
Fix what failed, then run `farm check` again with just the jobs that failed (`farm check lint
test`) and, if you want the whole gate green once more, once without arguments.
One whole `farm check` per commit-worthy state.

If a golden file changed on purpose, bless it and read every diff:

```sh
farm bless -p uses-ink-cli
```

Only the `*.snap` files that changed come back (never a `.snap.new`, never source), and the
list is printed. Read each diff (`jj diff <path>`) before you commit: a blessed snapshot is a
claim that the new output is right. A `.snap` you edited here after the bless started is never
overwritten: nothing comes back and the call exits 4; run it again once you are done editing.

Formatting comes back the same way:

```sh
farm fmt
```

It runs `cargo fmt --all` on any host and brings back only the `.rs` files whose bytes changed
there (never a `.rs` that rustfmt left alone, never another kind of file), and prints the list.
Run it instead of `cargo fmt` here: the farm is the only place you build, and a plain `farm cargo
fmt --all` formats the host's copy and loses the result. It takes no arguments and no `--detach`.
If you edit a listed file in the workspace after `farm fmt` started, it brings nothing back, names
the files, and exits 4; save your work and run it again. On the `local` host the files are
formatted in place and the list says which. Exit codes are the table above: 0 formatted (also
when nothing changed), 1 `cargo fmt` failed (a syntax error), 3 farm trouble, 4 refused.

## Never poll

Do not write `until grep ...` loops and do not call `sleep`. A job that may take long is
started with `--detach` and waited for with one blocking call:

```sh
farm --detach cargo nextest run --workspace
# prints: RUNNING as box-a.18c3f9a21; continue with `farm wait box-a.18c3f9a21`
farm wait box-a.18c3f9a21
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

## The test budgets

The 100 ms test budgets of `.config/nextest.toml` are for the owner's quiet machine. The farm's
hosts are shared: six agents building at once made them fail for want of CPU, and a pure-logic
test took 118 ms while four agents shared `here`. So every job runs with `NEXTEST_PROFILE=ci`
(unless your environment already sets a profile), whose 5 s cap still stops a hang. A slot gives
each job its own cores for as long as it runs, so a test that takes seconds is slow. The farm
never retries a failed test and never changes the nextest config; the 100 ms budgets are checked
on a quiet machine.

## Golden hosts

Output-affecting tests and goldens run only on a golden host, which by default means x86_64.
Other architectures take the jobs the file does not mark `golden`: here `lint`, `docs` and `wasm`. A
golden job that cannot find a golden host fails with exit 3 and says so; one pinned with
`--host` to a host that is not golden is refused with exit 4.

## What a farm run builds

Every farm run of a host uses the same environment, so the host's `target/` stays warm:

- `CARGO_PROFILE_DEV_DEBUG=line-tables-only`: debug info with line tables only. The build and
  the link are smaller and faster, and panics and backtraces still show file and line. You lose
  local variable inspection in a debugger, which farm runs are not for.
- `NEXTEST_PROFILE=ci`, unless the caller's environment already sets a profile. The 100 ms
  budgets of the default profile are for the owner's quiet machine. The farm's hosts are shared
  (a pure-logic model test took 118 ms while four agents shared `here`), so a job runs the `ci`
  profile, which keeps a 5 s cap against hangs. A test that is over 100 ms on the farm is
  therefore no longer a failure there; the budgets are checked on a quiet machine.
- `CARGO_TERM_COLOR=never`, `INSTA_UPDATE=no` (`always` for `bless`), `CARGO_BUILD_JOBS` and
  `NEXTEST_TEST_THREADS` set to the cores of one slot.
- `CARGO_TARGET_<HOST TRIPLE>_RUSTFLAGS=-C link-arg=-fuse-ld=mold` only when the host says
  `mold = true` and `RUSTFLAGS` is not already set there. It names the host's own triple: a plain
  `RUSTFLAGS` would reach `wasm32` too, where the linker is `rust-lld` and the component builds
  fail to link.
- `CARGO_TARGET_DIR` is never set: the target is `target/` of the synced workspace.
- Incremental compilation stays on.

The farm's artifacts are not a plain `cargo build`'s: the debug setting changes the profile, so
a `cargo build` in the same `target/` rebuilds everything once. On the host called `local` this
applies to your own `target/` too, because `local` runs in place. Use one or the other in a
workspace.

## Syncing

The first job of a workspace on a host seeds its directory (`<dir>/<repo>/<workspace>`) from the
newest sibling there by a reflink copy, so third-party crates are warm. Then `rsync -a --checksum
--no-times --delete` makes it equal to the workspace, so cargo rebuilds only the crates that changed.
Files are compared by content, and a file that was written gets the time of the write, not the
time it has here. This matters: cargo calls a crate fresh when its artifacts are newer than its
sources. A seeded tree holds artifacts built after the sibling's sources, and a source that a
rebase rewrote here can carry an older time than those artifacts. With `-a` alone rsync stamped the
changed file with that older time, and the host compiled the old code ("unresolved import", a
missing trait method) until every `.rs` file was touched. With the two flags a file whose bytes
changed is newer than every artifact, and a file whose bytes did not keeps its time, so nothing
rebuilds that did not change. Files brought back (`bless`, `fmt`) are written the same way. A tree
synced before this change may still hold such a stale artifact; `touch` the file once, or remove
`target/` on that host.
Excluded: what `.gitignore` ignores, `.jj`, `.git`, `.impeccable`, `target`, and `.env`,
`.env.*`, `*.pem` and `*.key`. The rsync target is always under the host's `dir`. A workspace is
one agent's: do not edit files while a job of that workspace is running unless you want the job
to see them.

State on a host is under `<dir>`: `slots/`, `queue/`, `jobs/<id>/`, `locks/`. `farm status`
removes job state older than seven days and says how much.

## New workspaces

Use `farm workspace` instead of `jj workspace add`:

```sh
farm workspace parser           # from main
farm workspace parser --at <rev>
farm workspace --forget parser
```

It adds `../uses.ink-ws/<name>` and seeds its `target/` with `cp -a --reflink=always` from the
newest warm sibling, so the workspace builds in seconds instead of ten minutes: the workspace's
crates rebuild, the third-party crates do not. Where the filesystem has no reflinks it says so
and copies in full. `--forget` refuses unless the path is a direct child of `uses.ink-ws`, is a
jj workspace of this repo, is not the one you are in, has no uncommitted changes and holds no
file outside `target/` that jj does not track (an ignored `.env`, say), since removing the directory would delete it.
The refusal names the first five such files. To allow losing a kind of file, such as a build cache,
list its name under `disposable` in the `[workspace]` table of `.config/farm.toml` (see The jobs
file); otherwise move the file out or commit it. `--forget` reads that file from the workspace you
run it in.

## Briefing an agent

Paste this into an agent's prompt:

> Build, lint and test only through `farm`, never with plain `cargo` and never with
> `ssh` or `rsync`. While working, run package-scoped commands: `farm cargo check -p
> <crate>`, `farm cargo clippy -p <crate>`, `farm cargo nextest run -p
> <crate> -E 'test(<name>)'`. Do not run the whole gate per edit. When a state is ready to
> commit, run `farm check` once and fix what it reports; if a golden file changed on
> purpose, run `farm bless` and read every diff it lists. Format with `farm fmt`, never with
> plain `cargo fmt`. Never poll and never write
> sleep loops: start long jobs with `--detach` and block with `farm wait <id>`; exit
> code 75 means still running, call `wait` again. If the queue says you are waiting, wait: the
> slots keep jobs from starving each other, so do not retry a test to get past a failure. Make a workspace with `farm workspace <name>`, not `jj
> workspace add`. Exit codes: 0 passed, 1 failed, 2 usage, 3 farm trouble, 4 refused, 75 still
> running, 124 hit its limit.
