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
inputs = ["**", "!docs/**"]      # optional: the files the job reads (see The result cache)
cache = false                    # optional: never reuse a pass of this job
max_load = 6                     # optional: place it only on a host whose load average is at most this (see Where a job goes)
affected = true                  # optional: `farm check test --affected` runs it as nextest for the affected crates
```

An optional `[workspace]` table lists names that `farm workspace --forget` may delete although jj
does not track them (see New workspaces):

```toml
[workspace]
disposable = [".ink"]            # file or directory names, matched at any depth
```

Six more optional tables (all keys are typed; an unknown key or a zero is an error):

```toml
[sync]
exclude = [".jj", ".git", ".impeccable"]  # names the sync leaves out, at any depth; default [".jj", ".git"]

[cache]
max_age_hours = 168              # passes older than this are forgotten; default 168
max_entries = 256                # the oldest beyond this many are forgotten; default 256

[budget]
full_checks_per_day = 3          # `farm check` warns after this many a day; default off

[affected]
base = "main"                    # `farm test` measures the change from here; default: where the workspace left `main`
ignore = ["docs/**", "*.md"]     # paths in no crate that no test reads, as globs; default none

[history]
max_age_hours = 336              # timings older than this are forgotten; default 336 (two weeks)
max_samples = 2000               # the oldest beyond this many are forgotten; default 2000
```

`target`, `.env`, `.env.*`, `*.pem` and `*.key` are never synced and are built into the farm.

A disposable name is letters, digits, `-`, `_` and `.`, at most 64 characters, with no slash, and
not only dots; any other key under `[workspace]` is an error. This repo declares `.ink`, the
fixture builds' cache.

Jobs run in the order of the file. A job name is lowercase letters, digits, `-` and `_`, at most
32 characters. `farm check` with no names runs every job at once; `farm check lint test` runs
those, in that order. This repo declares `lint`, `test`, `docs`, `wasm`, `sites` and `worker` (Workers in `workerd`, on hosts with node), the stages
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
wake = "aws-wake"       # optional; a command that starts the host (see Hosts that stop themselves)
wake_timeout = 240      # optional, with wake; seconds to wait for the host to answer; default 240
wake_cost = 120         # optional, with wake; seconds a start adds to placement's estimate; default 120
handicap = 0            # optional; seconds added to the host's estimate for every job; default 0 (see Where a job goes)
test_threads = 4        # optional; the most nextest threads one job gets here; default is the slot's share of the cores

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
- `test_threads` caps the threads nextest starts for one job, below the slot's share
  (`cores / slots`). A 16-core host with two slots shares 8 threads per job, and 19% of the
  runs on such a host still missed the test budgets at a load of 15 (decision 0081). The farm sets
  `NEXTEST_TEST_THREADS` to the lower of the two, which nextest reads, and which also reaches the
  nextest that `cargo xtask check test` starts. A cap above the share changes nothing, and zero is
  an error. `CARGO_BUILD_JOBS` keeps the share.
- The arch is read on every probe, so there is nothing to cache or to go stale.
- `wake`, `wake_timeout` and `wake_cost` are for a host that stops itself when idle and costs
  money while it runs. They are only valid together with `wake`, and not on `ssh = "local"`. See
  Hosts that stop themselves.
- Every host needs a POSIX login shell, `flock`, `timeout`, `tail`, `setsid`, `rsync`, `cargo`
  (rustup, in `~/.cargo/bin`) and `cargo-nextest`. The wasm stage needs the `wasm32` targets,
  `wasmtime` and `cargo-deny` as `cargo xtask check` does. A host on a copy-on-write filesystem
  (btrfs, xfs with reflinks) seeds a new workspace in seconds.

## Verbs

| Verb | What it does |
| --- | --- |
| `farm check [job...]` | The repo's jobs from `.config/farm.toml` (here `lint test docs wasm sites worker`), all placed at once. |
| `farm cargo <args...>` | Any cargo command on a placed host, in the synced workspace. |
| `farm bless [nextest args]` | Nextest with `INSTA_UPDATE=always` on a golden host; brings back only changed `*.snap`. |
| `farm test [nextest args]` | Nextest for the crates the change affects and their dependents, on a golden host. `farm check test --affected` is the same for a job that declares `affected = true`. See Testing only what changed. |
| `farm fmt` | `cargo fmt --all` on a placed host; brings back only the `.rs` files whose bytes changed. |
| `farm status` | Each host: arch, load, slots used, queue, its running and queued jobs, the median seconds by job that placement uses, and where each declared job would go now. `--json` prints the same. |
| `farm workspace <name>` | A new jj workspace with a warm `target/`. `--forget <name>` removes one safely, and its copy on every host. `--prune` lists, and with `--yes` removes, the host copies no workspace here owns. |
| `farm wait <id>` | Blocks until a detached job ends, or the timeout passes. |
| `farm log <id>` | A job's log from its host; `--tail N` for the end. |

Flags go before the verb, or right after it; for `cargo` and `bless` everything after the first
word that is not a farm flag belongs to cargo, verbatim, a `--` and what follows it included:
`farm cargo clippy -p x --all-targets -- -D warnings` runs exactly that. A `--` before the first
cargo word only ends the farm's flags (`farm cargo -- --json nextest`), and is not passed on.

| Flag | Meaning |
| --- | --- |
| `--detach` | Start the job and print its id; do not wait. |
| `--timeout N` | How long one call blocks, in seconds. Default 540: a tool call may take 10 minutes. |
| `--limit N` | The hard limit of the job's command, enforced on the host by `timeout`. Default: the job's own `limit`, else 1500. |
| `--host NAME` | Use only this host. A golden job on a host that is not golden is refused. |
| `--json` | Print the machine report, schema `uses.farm/v1`; `status` prints schema `uses.farm/status/v1`. |
| `--verbose` | Print every command the farm runs, exactly. |
| `--version` | Print the version and the source hash the binary was built from. |
| `--fresh` | Run the job although an identical one passed before (the result cache is skipped; the new pass is still recorded). |
| `--budget N` | `check`: warn after N full checks in 24 hours in this workspace; else the `[budget]` setting. |
| `--affected` | `check`, `test`: run the named jobs that declare `affected = true` as nextest for the affected crates. `farm test` always is; the flag is accepted there so the two forms read alike. Never implied for `farm check`; the full `farm check` stays the gate. |
| `--base REV` | `test`, `check --affected`: measure the change from this revision; else `[affected] base`, else where the workspace left `main`. |
| `--keep-going` | `check`: let every stage finish when one fails, instead of cancelling the others. |
| `--prune`, `--yes` | `workspace`: list the host copies no workspace here owns; with `--yes`, remove them. |
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

## Looking at the built pages

`cargo xtask shoot` draws the pages `ink` builds in a real headless Chromium and brings the pictures back
(decision 0240). It is not a stage of the gate and not a job of `.config/farm.toml` (every job there runs in
`farm check`); run it by name on a host with node and network, which the x86_64 build hosts have:

```sh
farm --detach cargo xtask shoot site --width 1440,390   # landing and docs, built and served by ink
farm wait <id>
```

| Target | What it takes pictures of |
| --- | --- |
| `site` | `landing/` and `docs/` (home, `cli`, the decisions index, decision 0180), each built with `ink build` and served with `ink serve` on loopback |
| `dashboard` | `ink serve --data` with an empty data folder, signed out |
| `mock` | the design mock, from `--from <folder>` (a folder you copy in for the run; the mock is git-ignored and never committed), for a reference from the same engine |
| `probe` | nothing: whether a browser starts here, and which |
| `lock` | nothing: prints `tools/shoot/package-lock.json` between markers, to be committed |

`--width` takes up to eight widths (default `1440,390`). The PNGs are written under `target/shots/` on the
host and printed into the farm's log as base64 between `=====SHOT <name> <bytes> <width>x<height>=====` and
`=====END <name>=====` lines, since the log is the only way back; a page taller than 9000 pixels is cut there.
The log also says, per page and width, which fonts loaded, the page's height, any horizontal overflow with the
elements past the edge, and the browser's console messages and failed requests. Decode the log with a small
script of your own: the blocks are plain base64 of a PNG.

The browser is Playwright's `chromium-headless-shell`, downloaded into `~/.cache/uses-ink/browsers/` of the
host by the packages pinned in `tools/shoot/` and installed with `npm ci` under `target/shoot/node/`. It needs
no system package and no sudo on the hosts tried; `@sparticuz/chromium` is the fallback when it does not
start. The mock is not in the repository: put it in the workspace under an ignored path and pass `--from`.

## The inner loop

While you work, run the narrowest command that answers the question, scoped to a package:

```sh
farm cargo check -p uses-ink-engine
farm cargo clippy -p uses-ink-engine --all-targets
farm cargo nextest run -p uses-ink-cli -E 'test(parse)'
farm test
```

On a warm target a package-scoped check or test takes seconds. Never run the whole gate after an
edit.

The terminal gets a header (host, slot, time queued and run, the command), then a summary. For
nextest it is the counts and every `FAIL` or `TIMEOUT` test with its output, at most 60 lines
each. For cargo it is the number of errors and warnings and the first ten in short form. For a
job that prints `check:` lines it is those lines. The full log is streamed to `target/farm/<host>.<token>.log`.

## The result cache

A job whose command and inputs are the same as a job that already passed is not run again. The
farm prints the earlier result and exits 0:

```text
== test: petal, queued 0 s, ran 412 s
   $ cargo xtask check test
passed earlier on this tree (petal.18c3f9a21, 3 min ago)
log /home/me/code/uses.ink-ws/w/target/farm/petal.18c3f9a21.log
```

The key is the job name, its command line and variables, the digest of its inputs, the host
architecture (for `golden` jobs only), the channel of `rust-toolchain.toml` and the text of the
job file. Rules (decision 0075 says why):

- **Only passes are cached.** A failure is always run again.
- **A flaky pass is not cached.** A passing log with a `FLAKY`, `RETRY`, `TRY n` or `TIMEOUT` line
  is not kept, nor a job marked `cache = false`, nor a pass of a foreground job whose tree changed
  while it ran.
- **What is cached.** The jobs of `farm check`, and `farm cargo` with `check`, `clippy`, `test`,
  `doc`, `deny` or `nextest run`. `bless`, `fmt`, `build`, `run` and `install` never are.
- **The digest is of file contents,** over the files jj tracks (a new file counts at once), minus
  what the sync leaves out. A rebase that leaves the bytes alone hits the cache; an edit of one
  byte of an input misses; `farm fmt` and `farm bless` change the digest by changing files. The
  hashes of unchanged files are kept in `target/farm/hashes.tsv` (by path, size, mtime and
  inode), so a repeat digest is a stat per file.
- **`inputs` narrows what a job depends on.** The digest covers the files that match the globs,
  plus `Cargo.lock`, `rust-toolchain.toml` and `.config/**`; without `inputs` it covers the whole
  tree. A glob list is read in order and the last match wins. `*` and `?` stay in one path segment,
  `**` is a whole segment of any depth, a leading `!` takes files out, a leading `/` or any other
  `/` anchors the glob at the tree root, and a glob with no `/` matches a name at any depth. So
  `["**", "!docs/**", "docs/workflow.md"]` is everything but docs, except one file. A glob that is
  not one (`[`, `{`, `\`, `..`, an empty segment) is an error that names the job file and line.
  In this repo a docs-only change leaves `test`, `docs`, `sites` and `wasm` cached (`test` still
  reads `docs/workflow.md`), and `wasm` reruns only for `freehand`, `freehand-web`, `fixture`,
  `token`, `xtask` or the manifests. The other Rust jobs are not narrower than "everything but
  prose": `docs` is the doc tests of every crate and `sites` builds nearly all of them.
- **It is local and shared.** Passes live in `$XDG_CACHE_HOME/uses-ink/farm/<repo>` (else
  `~/.cache/uses-ink/farm/<repo>`), so a workspace rebased onto a main-equal tree finds the passes
  of the others. They expire by age and count (`[cache]`).
- **`--fresh`** on any verb skips the lookup.

### Identical jobs run once

Starting a job while one with the same workspace, command and input digest runs does not start a
second copy: the call prints `an identical job is already running, <id>; attaching to it` and
waits for it as `farm wait` does (and exits 75 past `--timeout`, with the same id). A detached job
that ends under `farm wait` or an attach records its pass. Two copies of one command never run in
one slot at once.

### Seeing the use

`farm check` ends with one line on stderr:

```text
farm: check: 2 stages run, 3 served from cache, 0 deduped, 412 host seconds
```

`farm status` adds, per workspace of the repo, the last 24 hours:

```text
last 24 h, farmcache: 3 full checks, 9 stage runs, 4 cache hits, 1 deduped, 1210 host seconds (this one)
```

`--budget N` (or `[budget] full_checks_per_day`) makes `farm check` print a one-line warning when
the workspace has run more than N full checks in a day that ran a stage. It never refuses. The
count lives in `<cache>/journal.jsonl`.

### Slots are the wrapper's alone

A job's command starts with the slot's lock descriptor closed, so a child that outlives the job
(a test server) cannot keep the slot, and when the command ends the wrapper kills the job's
process groups (its own and `timeout`'s) after a short grace. `farm status` lists each busy slot's
holder pid and age, says when that process is gone (something else keeps the lock), and marks a
slot `wedged?` when it has been busy for over five minutes on a host whose load is under 1.0 or
whose holder is gone.

## The finish

When a state is worth a commit, run the gate once:

```sh
farm check
```

The jobs run at the same time on different slots, so the wall time is the slowest job, not the
sum. The report names each job, its host and its time, and gives detail only for the failures.
Fix what failed, then run `farm check` again with just the jobs that failed (`farm check lint
test`) and, if you want the whole gate green once more, once without arguments.
When a stage of a full `farm check` fails, the stages still running or queued are cancelled, and
one line says which (`check: lint failed, so test and sites cancelled`). A cancelled stage is
reported as `CANCELLED` (`outcome: "cancelled"` in `--json`), is neither a failure nor a pass, is
never cached and adds nothing to the exit: the failed stage sets it. Its client is dropped, which
makes the host stop the job exactly as a lost connection does. `--keep-going` lets every stage
finish. A check with job names (`farm check lint test`) and an `--affected` check never cancel,
and a stage that attached to another call's identical running job waits for it, since that job is
not this call's to stop.
One whole `farm check` per commit-worthy state, and the cache serves any stage that already
passed on the same inputs, so a second one on an unchanged tree runs nothing.

### Testing only what changed

`farm test` runs nextest for the crates your change touches and the crates that depend on them,
instead of the whole workspace. It prints one line first, for example `affected: 5 of 52 crates
(changed: model, cli; dependents: 3)`, then runs `cargo nextest run -p a -p b ...` on a golden
host. Arguments after `test` go to nextest (`farm test -E 'test(parse)' --no-fail-fast`).

How the crates are found:

1. The changed files are `jj diff --summary` from the base to the working copy. The base is
   `--base REV`, else `base` under `[affected]` in `.config/farm.toml`, else the parent of the
   workspace's first commit above `main`. A rename counts its old and its new path.
2. A host runs `cargo metadata --no-deps --format-version 1` in the synced workspace. Each
   changed path belongs to the crate whose directory is its longest prefix. Dependencies between
   workspace crates (normal, dev and build) give the dependents, and the closure is taken, so a
   change to `model` tests everything that uses it, directly or not.
3. `Cargo.toml` at the root, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, and anything under
   `.config/` or `.cargo/` affect all crates, and the run is `--workspace`. So does a path in no
   crate (a new directory, a root fixture), because any test may read it, unless the repo lists it
   under `[affected] ignore` as globs (prose, licences). A change of only ignored paths prints
   `no crate affected`, starts no job and exits 0.

`farm check test --affected` does the same for a job marked `affected = true` in its `[job.*]`
table, under the job's name and limits; `--affected` needs the job names and fails for a job
that is not marked. Nothing applies it on its own: `farm check` with no arguments is the gate
and runs every stage in full, and a stage served from the result cache is never narrowed.

`farm test --affected` and `farm test --base REV` are accepted, the first as a no-op, so the
spelling of `farm check test --affected` works here too.

`farm test` is for the loop while you work. It does not replace the gate: a change a dependency
graph cannot see (a build script reading a file, a test reading a sibling crate's source) is
caught by the one full `farm check` at the end.

### Heavy-command budget for agents

The owner measured a day of agents: 16 full gates, `test` 50 times, `sites` 38. The rules:

- **One full `farm check` per task, at the end.** Meanwhile run `farm test` (the crates you
  changed and their dependents) or `farm cargo nextest run -p <crate>`, and `check` or
  `clippy` for the crate you changed. A whole-workspace nextest is never a loop command.
- **Pipeline, do not idle.** Start a long job with `farm --detach`, keep working, and collect it
  with `farm wait <id>`. Measured: half of an agent's wall time was waiting on a job it started.
- **Batch.** One multi-file edit per area; read files in parallel; one `grep` with alternation.
  5,300 small Bash calls averaged 4 s each.
- **A failed stage is rerun alone:** `farm check lint`, not the whole gate.
- **Nothing heavy after a clean rebase** except `farm check lint`. If the bytes did not change the
  cache answers anyway; if they did, run the stage that covers it.
- **Never start the same job twice.** If a call moved to the background, wait for its notice or
  `farm wait <id>`; a second copy attaches to the first, but it still costs a call.
- Do not use `--fresh` to get past a failure; it is for a pass you doubt.

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

### The Bash tool has its own timeout

An agent's Bash tool moves any call to the background after 2 minutes by default, and also
when a message reaches the agent while the call runs. A `farm` call blocks for up to 9 minutes,
so most gates cross that line. Two rules keep the agent out of trouble:

1. Pass `timeout: 600000` to the Bash tool on every `farm` call that may run past 2 minutes
   (`check`, `cargo nextest`, `cargo clippy`, `bless`, `wait`).
2. When the tool reports "moved to the background", the call is not lost. Its result arrives as
   a completion notice. Do nothing in the meantime: do not read the output file in a loop, do
   not write `pgrep`, `until`, `while` or `sleep`, and do not start the same job a second time.
   Either end the turn with one line saying what you wait for, or block once with
   `farm wait <id>` and `timeout: 600000`.

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

## Where a job goes

`farm` picks the host that should **finish** the job first, from how long that kind of job took
on each host before (decision 0076). Only hosts that may run the job are considered (a `golden`
job needs a golden host), and `--host` forces one. For each candidate:

```
expected finish = wait for a free slot + the host's median for the job
wait            = jobs ahead x the host's median / its slots      (0 with a free slot and no queue)
```

- **The key** is the stage name for `farm check` jobs, `bless` and `fmt`; for `farm cargo` it is
  the subcommand and how many packages it names (none or `--workspace`, one `-p`, several), so
  `clippy -p a` and `clippy -p b -- -D warnings` share their history. The median is over the
  host's latest 15 passes of the key; failures are recorded but not timed, because they stop early.
- **An unknown key on a host** takes the median of the other hosts' times scaled by core count
  (a host with half the cores is estimated twice as slow). A host with **no history at all** is
  tried once, as soon as it has a free slot, so its history fills in.
- **A host far over its cores** (load above 1.5 times the cores) has its estimate multiplied by
  the load per core, so a thrashing host is avoided.
- **With no timings for the key anywhere** placement is the old rule: a free slot and the fewest
  slots in use, then the lowest load, then the shortest queue.
- **A handicap** (`handicap = <seconds>` on a host) is added to the host's expected finish, so the
  host is used only when it finishes earlier than every other candidate by more than that. To keep
  the machine you work at free, give it `handicap = 120` in the hosts file, under its
  `[[host]]` with `ssh = "local"`. The estimate shows it, `here (est 90 s, queue 0, of which 60 s
  handicap)`. A handicapped host is not tried once for lack of history, and with one candidate
  the handicap changes nothing (decision 0080).
- **A load ceiling** (`max_load = <load average>` on a job) leaves out every host whose one-minute
  load average is above it, before any estimate. With every candidate over it the job waits:
  the farm says so once, looks again every 10 s and fails with exit 3 after 300 s, naming each host
  and its load. `farm status` shows the same reason on the `next <job>:` line. `--host` ignores the
  ceiling, and `farm cargo` commands have none. This repo sets `max_load = 6` on `test`, whose
  100 ms budgets a loaded host breaks.
- Ties go to the host with fewer slots in use, then to the one listed first. The choice is
  deterministic for the same history and probes.

The choice is one line on stderr, `farm: test: petal (est 30 s, queue 0) over here (est 64 s)`:
the estimate is seconds to the end including the wait, `queue` is the jobs ahead of this one.
`farm status` prints, per host, `median pass seconds by job` with the run count, and `next <job>:`
with where each declared job would go with the hosts as they are now; `farm status --json` has the
same under `hosts[].medians` and `next[]`.

The timings are one JSON object per finished job in `history.jsonl` of the repo's cache directory
(`$XDG_CACHE_HOME/uses-ink/farm/<repo>`, else `~/.cache/...`), shared by every workspace:
`{"at":..,"key":{"kind":"stage","name":"test"},"host":"petal","secs":30,"passed":true}`. It is
append-only and bounded: each run drops lines older than `max_age_hours` or beyond the newest
`max_samples` (the `[history]` table) and rewrites the file when it did. A line cut short by a crash
is skipped. Deleting the file only resets the estimates. A job that ends under `farm wait` or by
attaching to an identical running job is not recorded.

### Hosts that stop themselves

A host with `wake = "<command>"` may be asleep: a cloud machine that stops after idle minutes and
costs money while it runs. When its probe gets no answer it is **asleep**, not down:

- `farm status` lists it as `asleep` with what waking costs, and it is not an error.
- Placement counts it as a candidate with `wake_cost` seconds (default 120) added to its expected
  finish, so it is used only when the hosts that are up are queued or slower by more than that. A
  short job is never worth waking a host for. It is never the host that gets tried once for lack of
  history, and with no timings anywhere it is used only when nothing else may run the job.
- Placement needs its size, which a sleeping host cannot say: the farm remembers the arch, cores
  and slots of the last time it was up in `seen.json` of the cache directory. A host never seen up
  is placed on only with `--host`, which wakes it once.
- When it is chosen the farm runs `<command> <host name>` on this machine, no shell, killed after
  `wake_timeout` seconds (default 240), then probes every five seconds until the host answers or
  those seconds are over. The host name is the only argument the command gets: with `wake =
  "aws-wake"` it runs `aws-wake farm-a`, so the script takes the name as `$1`. All hosts chosen for
  one call are woken at once, and the jobs start after that.
- A wake that fails (the command exits non-zero, cannot start, hangs, or the host still does not
  answer) says one line naming the command and its exit status, and the host is left out of this
  job's placement; the job is placed again on the rest. With `--host` it is the error (exit 3).

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
- `CARGO_TERM_COLOR=never`, `INSTA_UPDATE=no` (`always` for `bless`), `CARGO_BUILD_JOBS` set to
  the cores of one slot and `NEXTEST_TEST_THREADS` to the same, or to the host's `test_threads`
  when that is lower.
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
sibling there whose `target/` a job used last, by a reflink copy, so third-party crates are warm.
Every job touches the `target/` of its tree as it starts; a sibling that has built nothing (no
`target/debug` or `target/release`) is never the source, and with none the first build is cold and
the farm says so. The copy is of the whole tree, not of `target/` alone, on purpose: unchanged
sources keep the times they had beside their artifacts, so the workspace's own crates that did not
change stay fresh (decision 0081). Then `rsync -a --checksum
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
Excluded: what `.gitignore` ignores, the names of `[sync] exclude`, `target`, and `.env`,
`.env.*`, `*.pem` and `*.key` (the repo's own names are `[sync] exclude`; here `.jj`, `.git` and `.impeccable`). The rsync target is always under the host's `dir`. A workspace is
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

`--forget` also removes the workspace's copy on every host (`<dir>/<repo>/<name>`, by default
`~/farm/uses.ink/<name>`), once the local removal has succeeded. The hosts are asked at once and
each says what it did: how much it removed, that it had no copy, or that it was not reached. A
host that is down keeps its copy; the command still succeeds, says so, and `--prune` removes it
later. A copy is removed only when its path, with symlinks resolved, is directly inside the
resolved `<dir>/<repo>` and that is inside the resolved `<dir>`; otherwise the host leaves it
alone, the command says which path and why, and exits 4. `local` has no copy and is not asked.

```sh
farm workspace --prune          # lists the copies no workspace here owns, and what they hold
farm workspace --prune --yes    # removes them
```

`--prune` lists, on every host, the directories under `<dir>/<repo>` whose name is not one of
this machine's workspaces (each directory of `uses.ink-ws`, `main`, and the one you run it in).
By default it removes nothing. It looks at this machine's workspaces only, so copies made by
another checkout of the repo that shares the hosts count as unowned: run it from the machine that
owns the farm. Half-made `.seed` copies and names the farm would not create are not listed.

## Briefing an agent

Paste this into an agent's prompt:

> Build, lint and test only through `farm`, never with plain `cargo` and never with
> `ssh` or `rsync`. While working, run targeted commands: `farm test` (nextest for the crates
> you changed and their dependents), `farm cargo check -p <crate>`, `farm cargo clippy -p
> <crate>`, `farm cargo nextest run -p <crate> -E 'test(<name>)'`. Never run the whole
> workspace's tests per edit. PIPELINE: start every long job with `farm --detach`, keep
> working, collect it with `farm wait <id>`, and never idle on a job you started. BATCH: one
> multi-file edit per area with Edit or Write, or one script; read files in parallel; one grep
> with alternation (5,300 small Bash calls averaged 4 s each). ONE full gate at the end: when a
> state is ready to commit, run `farm check` once and fix what it reports (a failed stage is
> rerun alone, `farm check lint`; after a clean rebase run nothing heavy except `farm check
> lint`; never start the same job twice: `farm` serves a stage that already passed on the same
> inputs and attaches an identical running one, and `--fresh` is not a way past a failure). If a
> golden file changed on purpose, run `farm bless` and read every diff it lists. Format with
> `farm fmt`, never with plain `cargo fmt`. Never poll and never write `pgrep`, `until`,
> `while` or `sleep` loops. Pass `timeout: 600000` to the Bash tool on every `farm` call that
> may run past 2 minutes. If the tool says a call was moved to the background, its result
> arrives as a notice: end your turn with one line, or block once with `farm wait <id>`; exit
> code 75 means still running, call `wait` again. If the queue says you are waiting, wait: the
> slots keep jobs from starving each other, so do not retry a test to get past a failure. Make
> a workspace with `farm workspace <name>`, not `jj workspace add`. Exit codes: 0 passed, 1
> failed, 2 usage, 3 farm trouble, 4 refused, 75 still running, 124 hit its limit.
