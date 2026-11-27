---
number: "0080"
title: "The farm keeps heavy jobs off a handicapped host and tests only on a quiet one"
status: accepted
date: 2026-11-28
area: farm
supersedes: []
---

## 0080 The farm keeps heavy jobs off a handicapped host and tests only on a quiet one

Accepted, owner. Two problems seen in one session. Heavy jobs ran on `here`, the owner's own
machine, although remotes were free. And the `test` stage failed with no bug in the code when the
test host was loaded: with `farm-a` and `farm-b` running several jobs at once (load 15 on 16
cores) the CLI tests missed their 100 ms and 5 s budgets, and one push end-to-end test failed with
`auth.signature.skewed`, 8 to 10 s of apparent clock skew. Extends 0047 and 0076.

- **`handicap` on a host.** `handicap = <seconds>` in the hosts file is added to the host's
  expected finish for every job (0076). The host is chosen only when it finishes earlier than
  every other eligible host by more than the handicap, so it is used when the others are down,
  asleep for longer than the handicap, or queued deeper than it. With one eligible host the
  handicap changes nothing. A handicapped host is also never the one tried once for lack of
  history (its estimate is scaled from the others), and with no timings anywhere the least loaded
  rule prefers a host without one. The estimate says it: `here (est 90 s, queue 0, of which 60 s
  handicap)`. The default is 0. Seconds were chosen over a boolean `avoid` because the owner can
  say how much slower a remote may be before `here` is worth using, in the same unit as the
  estimate, and over a `weight` because a multiplier means nothing for a job with a 3 s median.
  The owner adds, under `here` in `~/.config/uses-ink/farm.toml`:

  ```toml
  [[host]]
  name = "here"
  ssh = "local"
  handicap = 120
  ```

- **`max_load` on a job.** `max_load = <load average>` in `.config/farm.toml` is a whole number:
  the job is placed only on a host whose one-minute load average, as the probe reads it, is at
  most that. Hosts over it are left out before the estimate, even when they would finish first,
  and a sleeping host counts as idle. When every host that may run the job is over, placement
  returns `Error::Loaded` naming each host and its load; `farm` looks again every 10 s and fails
  with that error after 300 s (exit 3) rather than run the job where it would flake. `farm
  status` shows the same text on the `next <job>:` line, so the owner sees why a job waits and
  for what. `--host` still forces and ignores the ceiling: the user chose the host. In a
  `farm check` the whole placement waits, so the other stages start when the test stage can,
  not before. A ceiling applies to the declared jobs only, not to `farm cargo` commands; a
  `farm cargo nextest run` on a loaded host can still flake.
- **The value for `test` is 6.** The budgets are fixed (100 ms for most tests) and the host
  runs `cores / 6` slots with a job per slot, so one other job building leaves a host at 6 to 8.
  The failures were seen at 15 on 16 cores. At 6 a host is at most about a third busy; it is
  a guess to be tuned against `farm status` if tests still flake or the wait grows.
- **The skew is not clock skew.** `chronyc tracking` on `farm-a`, `farm-b` and `petal` shows
  offsets under 0.2 ms of NTP time, so the hosts agree. The push test starts `ink serve` and the
  `ink` client on the same host, so both read one clock, and passes `--auth-skew 1`: a window of
  one second either side (`cli/tests/push.rs`; the default of the API is 60 s). A request whose
  `created` differs from the server's time by 8 to 10 s was therefore stamped, then held for 8
  to 10 s before the server read it, which is what a process starved of CPU does at load 15 on 16
  cores (the client is descheduled between signing and sending, or the server between accept and
  verify). It is an artifact of a stalled process under a one second window, not a signing fault,
  and the signing code is unchanged. `max_load` on `test` addresses the load; widening the test's
  window is the owner's call and is not done here.
