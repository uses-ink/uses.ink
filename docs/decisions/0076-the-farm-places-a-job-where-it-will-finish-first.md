---
number: "0076"
title: "The farm places a job where it will finish first, by what that job took before"
status: accepted
date: 2026-11-26
area: farm
supersedes: []
---

## 0076 The farm places a job where it will finish first, by what that job took before

Accepted, owner. 994 job records from one day on three hosts showed the placement of 0047 (a free
slot, the fewest in use, the lowest load) putting a stage on a host two to eight times slower than
one that was idle or nearly so: `lint` took a median 3 s on the aarch64 host, 51 s on petal and 73 s
on the large host; `test` 30 s on petal and 64 s on the large host, with a p90 of about 200 s on
both. Load and slot counts say nothing about speed. Extends 0047 and 0075.

- **The rule.** Among the hosts that may run the job, the lowest expected finish: the wait for a
  free slot (jobs ahead times the host's median for the job, over its slots) plus the median.
  `--host` still forces. A host far over its cores is slowed in the estimate; with no timings for
  the job anywhere the rule is the old one. Integer arithmetic and a fixed tie-break make the
  choice deterministic, and it is tested with fake histories and fake probes, no ssh.
- **The key is the kind of job, not the command line.** A stage name, or a cargo subcommand and
  how many packages it names. The whole command line would make every `-p` a new job with no
  history.
- **History is a local append-only file** of typed samples (job key, host, seconds, passed),
  bounded by age and count in `[history]`, shared by workspaces, and only passes are timed. It is
  not a database, nothing daemonised reads it, and losing it only resets the estimates.
- **A host with no history is tried once** while a slot is free, and an unknown key on a host is
  estimated from the others scaled by cores, so history fills in without a benchmark step.
- **The choice is said**, on one line per job and in `farm status` (also `--json`), with the
  estimate and the runner-up, so an owner can see why and correct the config if it is wrong.
- **Hosts that stop themselves** (`wake`, `wake_timeout`, `wake_cost`) are candidates with the
  start cost added to their estimate, so a machine that bills by the minute runs only when it saves
  more than it costs to start; a failed wake is a typed error and the job is placed without it. The
  farm wakes, and never stops, a host.
- **`farm cargo` passes every argument after the subcommand verbatim**, `--` included. The parser
  dropped the first `--` and everything after it was cargo's anyway, so `-- -D warnings` lost its
  lints and agents wrote `farm cargo -- clippy ... -- -D warnings`; that form still works.
