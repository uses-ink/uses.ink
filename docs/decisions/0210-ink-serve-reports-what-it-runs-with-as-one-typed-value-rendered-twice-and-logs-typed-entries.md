---
number: "0210"
title: "`ink serve` reports what it runs with as one typed value rendered twice, and logs its work as typed entries"
status: built
date: 2026-11-28
area: cli
supersedes: []
---

## 0210 `ink serve` reports what it runs with as one typed value rendered twice, and logs its work as typed entries

Built (2026-11-28). The start of `ink serve` was a flat list of `label  sentence` lines with the numbers inside
the prose, and its log was `<timestamp> <Display>` with no level, no component and no colour. Both are now typed
data that goes through the terminal layer of decision 0061.

- **One `Startup` value, two forms.** `crates/surface/cli/src/selfhost/report/startup` builds a `Startup`: the program and
  version, the listen address, the three addresses a person opens (site, dashboard, API), the first-run link, and
  sections of rows. A row is a key, a typed `Value` (a URL, a path, a count, a size in bytes, a length of time, a
  rate, a flag, a secret, a list), a `Source` and an optional `Caution`. The human report (`Render`) and the JSON
  document (`Serialize`, schema `uses.serve/v2`) are written from that one value, and a test checks that the text
  holds every row and address of the JSON. `uses.serve/v2` keeps `address` and `mode` of v1.
- **Values are painted by what they are.** A size is written by `model::Bytes` (`256 MiB`, not `268435456 B`) and a
  length by `Span` (`10m`, `1h 30m`, `180d`); the JSON carries the raw number and the unit. Two roles were added to
  the closed set of decision 0061, which is a design change and is made here: `Link` (cyan, underlined) for an
  address to open and `Figure` (magenta) for a number, a size or a length of time. The role table of `docs/cli.md`
  section 5 and the golden of the role table changed with them.
- **Where a value came from.** A setting that is an `Option` in the flags is `default` when it is `None` and names
  its flag when it is `Some`; a setting with a default value is compared to that default. No `ArgMatches` is kept.
  A value worked out from other settings (the origin from the listen address, the audience from the API host)
  says which flag overrides it.
- **A caution is a typed row property, with its fix.** A plain http origin makes the cookie not `Secure`; the row
  says so in the warning role and gives the fix. The retired owner token file of an older version is the same kind.
- **The first-run link is a block of its own.** It is a `Rail` with no right border, so it is never wrapped or cut
  by us, with how long it works and that it is shown once. A start that made no link says `not made` and why. The
  link is in the JSON document only when `--json` asks for it, as `admin.link`; before this, `--json` did not show
  it at all.
- **`--print-config`** prints the same report and exits, from the flags alone (`Plan::planned`). It does not go
  through `Product::open`, which creates the data folder, migrates the database and mints the first-run link: a
  config check must not use up the one link of a fresh folder. Its `admin` is `unminted`.
- **The last line is the sentinel.** The report ends with `press ctrl-c to stop`. It is written after the control
  plane is set up, at the default verbosity, and the end-to-end tests wait for it instead of the old `backing off`
  that sat inside the poll row. Stdout still holds the bare address as its first line.

The log is a second `Entry` type in `crates/surface/cli/src/selfhost/report/log`. What happens is a `Happens`: a level, a
component (`http`, `api`, `watch`, `worker`, `domains`, `mail`), a stable name such as `worker.built`, a message and
typed facts, and the server's events, the loops' events and the mail sender's notes each implement it, so the three
writers that were three formats are one.

- **Human** lines are a dim RFC 3339 time to the second, a coloured level, the component, the message and the facts
  as dimmed `key=value`, with the causes of an error under it and a `fix:` line where there is one (a loop that gave
  up). A request line reads `GET /tea 200 3ms`, its status coloured by class. On a narrow terminal the facts move
  to a line of their own and the message is cut with the usual mark.
- **JSON** (`--log-format json`, and `--json`, since stderr is a machine channel then) is one object a line: `time`,
  `level`, `component`, `event`, `message`, `fields`, and `causes` and `fix` when there are any. No colour.
- **What is shown.** `--log-level error|warn|info` (default `info`); `-q` lowers it to `warn` and `-qq` to `error`.
  An ordinary request (served, not found, an API call under 500) is shown only with `-v`. A refused or failed
  request is always shown, at `warn` or `error`. Builds, watch events, loops starting and stopping and mail are
  always shown at `info`. `--log-time utc|local` picks the zone of the time.
- **Tracing is another channel.** The tracing of decision 0065 (`--log`, `-vvv`, `INK_LOG_FORMAT`) is for debugging
  the program and is unchanged. `--log-format` has no environment variable, because `INK_LOG_FORMAT` is that
  channel's. With `-v` the tracing layer also turns on at `warn`, so a refused request can appear once in each.

**Why:** a first run is read by a person who needs three addresses and one secret, then the settings they may
change; and the same data had two writers that could drift. **Reversal:** none; v1 of the document is a subset of
v2.
