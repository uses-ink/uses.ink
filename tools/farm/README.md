# farm

`farm <verb>` is the build farm client. A repo's jobs, and any cargo command, run on the hosts of
your own config, each in a slot of its own, so several agents do not make one machine thrash and a
test budget is measured on a quiet core. It knows no project: the jobs are the repo's own
`.config/farm.toml`. The verbs and flags are in [the workflow](../../docs/workflow.md), and the
reason it is built this way is decision 0047.

## What it may use

Nothing of ours: it depends on no other crate of the workspace, and no crate depends on it. It
reads the repo as files (`.config/farm.toml`, the workspace's manifests), never through our types.
The list for every crate is in [the crate index](../../docs/crates.md).

## Where things are

Each directory of `tools/farm/src` is one concern, and `tools/farm/src/main.rs` lists them:

- `tools/farm/src/affected`: the crates a change affects, so `farm test` runs only those.
- `tools/farm/src/cache`: what the farm remembers on this machine (passes, running jobs, the
  journal, the digest of a job's inputs).
- `tools/farm/src/cli`: the command line, exit codes, console, errors and version.
- `tools/farm/src/config`: the hosts file, the repo's job file and its workspaces.
- `tools/farm/src/host`: a host, its probe, and the `Fleet` of members over a transport (local, ssh).
- `tools/farm/src/job`: a job, its events, its result and its log summary.
- `tools/farm/src/place`: which host gets a job, how long jobs took there, waking hosts.
- `tools/farm/src/run`: `Farm` and the verbs, one file each under `tools/farm/src/run/verb`.

`tools/farm/build.rs` computes the digest the farm's own `status` verb reports.

## How to test it

The farm's tests run its programs on the machine with the real `sh`, `flock` and `timeout`, and some
check the time a job is stopped at. `.config/nextest.toml` keeps the heavy ones from running at the
same time. Run them on a farm host, not on a shared laptop:

    farm cargo nextest run -p farm

`docs/workflow.md` has a test that every verb and flag is named in it, so a new verb needs a row there.
