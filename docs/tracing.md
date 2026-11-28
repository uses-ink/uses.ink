# Tracing: logging for debugging

Decision 0065. This page is the design; `logging/` is the code. The word "trace" already means the
replayed ladder of a decision (`ink explain --why`, `uses.trace/v1`). This page is about the
process: what it was doing, how long each part took, and why it refused. The crate name is the
owner's choice; docs and code say "log" for the output and "span" for a timed scope.

## What it is for

Three questions, asked by the person running `ink` or the process behind `ink serve`:

1. **What happened, in what order, and how long did each part take?** (a build, a request, a job)
2. **Why did it do that?** (why a page was re-rendered, why a request was refused, why a job failed)
3. **What did it cost?** (fuel and time of a plugin call, rows of a statement, bytes of a blob)

It is separate from the product's output. Results go to stdout. Diagnostics, notices and progress
go through the output layer (decision 0061: text on stderr, or NDJSON `uses.event/v1` under
`--json`). The log is a third channel with no overlap:

| Channel | Where | Under `--json` | Default |
|---|---|---|---|
| Result | stdout | one JSON document | on |
| Diagnostics, notices, progress | stderr | NDJSON events | on (notices with `-v`) |
| **Log** | **stderr, or a file** | **file only, never stderr** | **off** |

Nothing in the log is printed to stdout, and no log line is ever inside a `--json` document or
interleaved with the NDJSON events: with `--json` the log goes to `--log-file` or nowhere.

## One crate

`logging` (package `uses-ink-logging`) owns the subscriber. Every other crate instruments through it
and depends on nothing else for logging; no crate sets up a subscriber. It re-exports the macros
of `tracing` (`logging::info!`, `logging::warn!`, ...) and adds the typed parts below.

It has two features, so the lean part stays small:

| Feature | Adds | Who turns it on |
|---|---|---|
| none | `tracing`, the field types, `Secret`, the span macros, the names | every instrumented crate; this is all a wasm plugin sees through the engine |
| `layers` | `Logging`, `Filter`, the human, JSON-lines and timing layers | the CLI only |
| `capture` | `Capture`, the test subscriber | dev-dependencies |

Dependencies, with their licences against `deny.toml`:

| Crate | Licence | Why |
|---|---|---|
| `tracing` 0.1 (already in the lock) | MIT | the facade and its macros |
| `tracing-subscriber` 0.3, features `registry` and `std` only | MIT | span storage and `filter::Targets` |
| `sharded-slab`, `thread_local`, `lazy_static` (its dependencies) | MIT, MIT OR Apache-2.0 | none of ours |

Versions are the ones in `Cargo.toml` and `Cargo.lock`; this page names none. Left out on
purpose: `fmt`, `ansi` and `env-filter` (a second formatter, and a regex engine for a filter
language `Targets` already parses); `tracing-error` (below); the `attributes` feature, so there is
no `#[instrument]` (below).

**`tracing-error`, considered and not used.** A `SpanTrace` records the spans an error was made
in. Our errors are typed enums with `source()` chains and carry their own subject, and an error is
logged at the place that handles it, inside the spans that matter. The log line therefore already
has its span path; a trace inside the error value would repeat it and make every error type hold
one.

**`#[instrument]`, not used.** The attribute records arguments through `Debug`, which is the
"format a string at the call site" habit the owner rejects, and it names the span after a
function. Spans are opened by the typed macros below, in the layer they belong to.

## Typed fields

`tracing::field::Value` is sealed: a newtype cannot implement it. The crate's `Field` trait takes
its place. Each type says what it records as, and a call site names the type, never a format
string:

```rust
logging::debug!(name: logging::event::BLOB_IO, bytes = logging::Size(n).field(), created = !held);
```

The conversion is an argument of the `tracing` macro, so it runs only when the callsite is
enabled; a disabled event costs one level check. The types:

| Type | Records as | Notes |
|---|---|---|
| `Size(u64)`, `Count(u64)`, `Status(u16)` | unsigned number | `From<usize>` for the first two |
| `Took(Duration)` | whole microseconds, as `took_us`, `limit_us` | saturates |
| `Digest(&[u8])` | the first 6 bytes as hex | a short id for a log, never a URL |
| `Shown(T: Display)` | lazy text | ids, hosts, content paths |
| `Located(&Path)` | lazy text | a path as given |
| `Secret(T)` | `[redacted]` | see Redaction |
| `Failure(&dyn Error)` | an error, with its whole `source()` chain | see Errors |
| `Kind` implementors | a name constant | `Stage`, `Unit`, `Io`, and each crate's own enums |

Names are constants: span names in `logging::name::span`, event names in `logging::name::event`. Field
keys must be identifiers (a `tracing` rule), so they are listed here instead and kept few.

Keys used: `kind` (what tells spans of one name apart), `step`, `unit`, `build`, `job`, `site`,
`attempt`, `request`, `method`, `path`, `host`, `status`, `route`, `bytes`, `rows`, `changes`,
`fuel`, `took_us`, `limit_us`, `outcome`, `reason`, `key`, `digest`, `error`.

### Errors

An error is logged as `error = logging::Failure(&e).field()`. It reaches the layers as
`record_error`, and they walk `source()`: the human layer prints the message and one `caused by:`
line per cause, the JSON layer writes `{"message": ..., "causes": [...]}`. No call site joins a
chain into one string. `Capture` keeps the same structure, so a test asserts on the causes.

### Redaction

Redaction is by construction, not by care at the call site.

- `Secret<T>` prints `[redacted]` in `Debug`, in `Display` and as a field, whatever it holds. A
  unit test checks all four ways to print it.
- The API's `Bearer` token is a field only as `Secret`; the auth event says the outcome
  (`granted`, `missing`, `malformed`, `unknown`), never the header or the token.
- A request path is logged without its query (a token can sit in a query), cut at 200 characters.
  No header is logged. No body is logged. Paths in files are logged as given.
- A statement is logged by its shape (`select`, `insert`, ...), its row count and its change
  count, never its text and never a bound value.

## The layers of the span tree

Outermost first. Each is a macro in `logging` so the span's target is the module that opened it (an
area filter then governs the structure and the events together).

| Span | Macro | Level | Fields | Opened by |
|---|---|---|---|---|
| `command` | `command!` | info | `kind` (the verb) | the CLI |
| `build` | `build!` | info | `build` (id), `kind` (trigger) | `build/local` |
| `stage` | `stage!` | info | `kind` (a `Stage`), `step` | engine, `build/local` |
| `unit` | `unit!` | debug | `kind` (`file`, `page`, `generated`, `image`), `unit` | engine, image host |
| `plugin` | `plugin!` | debug | `kind` (the plugin), `fuel`, `took_us` | wasm host |
| `request` | `request!` | info | `request` (a counter), `method`, `path`, `host`, `status`, `took_us` | `edge/axum` |
| `api` | `api!` | info | `kind` (method), `route`, `status` | `edge/api` |
| `job` | `job!` | info | `job`, `site`, `attempt` | control plane |
| `statement` | `statement!` | debug | `kind` (a `Shape`), `rows`, `changes` | `db/sqlite` |
| `blob` | `blob!` | debug | `kind` (`read`, `write`, `remove`, `probe`), `digest`, `bytes` | `blob/dir` |

The stages, in the order a build runs them: `fetch` (the snapshot from its source: the git fetch,
the folder read), `discover` (scope, layout, ignores, the vault's settings), `source` (each kept
file read through its format), `decide` (pages, links, assets, routes, the assembled site),
`image` (raster images prepared), `render` (the artifacts), `write` (blobs stored). `step` names
the engine stage inside one (`root`, `layout`, `vault`, `files`, `read`, `pages`, `links`,
`embeds`, `assets`, `site`, `routes`, `assemble`, `guard`, `prepare`, `artifacts`, `blobs`,
`snapshot`).

A span that is async is entered with `.instrument(span)`, never by holding `enter()` across an
`await`. A span that fans out to threads is passed explicitly (the image workers take the
caller's span).

## Events

Events have constant names. The main ones, by area:

| Area | Events |
|---|---|
| build | `build.start`, `build.end` (status, `took_us`, error chain), `source.opened`, `source.fetched` |
| engine | `engine.interpreted` (counts), `engine.rendered` (reused, rendered), `unit.planned` (outcome, reason, key) |
| image | `cache.lookup` (`kind=image`, `outcome` memory/disk/miss/rejected, key), `image.decoded`, `image.resized`, `image.encoded`, `image.refused` (error chain) |
| plugins | `plugin.loaded` (compiled, cached, replaced), `plugin.refused` (fuel, time, memory, table, trap, with the failure chain) |
| edge | `edge.listening`, `edge.accepted`, `request.answered`, `request.refused`, `request.failed`, `response.cut`, `connection.failed`, `connection.lingered`, `edge.stopping`, `edge.stopped` |
| api | `api.auth` (outcome only), `api.answered` |
| control | `job.claimed`, `job.dropped`, `job.settled`, `job.failed` (retry or last), `deploy.published`, `deploy.rolled` |
| db | `db.migrated`, `db.statement`, `db.batch` |
| blob | `blob.io` |
| watch, worker | `watch.polled`, `watch.queued`, `worker.tick`, `git.head` (the head read), `task.started`, `task.crashed`, `task.restarting`, `task.gave_up`, `task.stopped` (supervision) |
| edge lookup, folder | `sites.serving`, `sites.asked`, `sites.placed`, `sites.failed`, `sites.stopped`; `folder.opened`, `folder.loaded`, `folder.refused` |
| queue, secret, cache | `queue.sent`, `queue.received`, `queue.settled`; `key.opened`, `key.refused`, `secret.sealed`, `secret.opened`; `cache.stayed` |

`unit.planned` says what the incremental planner did with one unit and why: `outcome` is `reused`
or `rendered`, `reason` is `hit` (the cache held the unit under its key), `miss` (it held nothing
usable) or `unkeyed` (no cache, or a theme with no identity). The cache is keyed by a digest of
everything the unit read, so the reason is how the lookup went and which key it used, not which
input changed. To learn what changed, compare `key` between two runs.

Levels: `error` for a failure (a failed job or request, a connection error), `warn` for a refusal
or a limit (a refused request or image, a stopped plugin call, a failed poll), `info` for one
event per unit of work (a request answered, a job settled, a deploy published), `debug` for the
steps inside it, `trace` for per-item detail (each resize). A healthy run writes no `warn`, which
is why `-v` adds no line to one.

## Filter, variables and flags

A filter is a list of `area=level` directives: `ink=debug,edge=trace,control=info`. A bare level
(`debug`) is the level of `ink`; a bare area (`edge`) is that area at `trace`. The most specific
directive wins, and a later one for the same area replaces an earlier one.

Areas: `ink` (all of ours), `engine`, `build`, `edge` (the edge, its adapters and the API),
`control`, `db`, `blob`, `host` (image, native, wasm), `plugin`, `watch`, `queue`, `cli`. Any
other name is a target prefix as written (`wasmtime=warn`). Areas stand for module paths
(`edge` is `uses_ink_edge...`), which is why a call site writes no `target:`.

| Variable | Flag | Meaning |
|---|---|---|
| `INK_LOG` | `--log <FILTER>` | the filter; also `[log] filter` in `config.toml` |
| `INK_LOG_FILE` | `--log-file <PATH>` | (`[log] file`) also write JSON lines to this file (append). A path like `/dev/fd/3` reaches an inherited descriptor; no code here opens a raw descriptor |
| `INK_LOG_FORMAT` | | (`[log] format`) stderr layout: `pretty` (default), `compact`, `json` |
| `INK_LOG_SPANS` | | `none`, `open`, `close`, `full` (default) |
| `INK_LOG_TIMINGS` | | `1` prints the span timing table at the end |
| `NO_COLOR`, `TERM=dumb` | | no colour |

Precedence, weakest first: the level the `-v` count stands for, the `[log]` table of the user's config file, `INK_LOG`, `--log` (the same order as `settings.rs`: flag, variable, file, default). Where two name
the same area the stronger one wins. `RUST_LOG` is not read: a farm host that sets it would turn
logging on in every test.

**`-v` keeps its meaning.** `docs/cli.md` says `-v` adds notices and `-vv` adds one line per file
and timings, in the output layer. That is unchanged. The log reads the same count as a floor for
`ink`: `-v` is `warn`, `-vv` is `info`, `-vvv` is `debug`, `-vvvv` is `trace`. `-v` therefore adds
nothing to a healthy run, and `-vv` adds the `info` events and the span tree on stderr, beside the
notices. cli.md said `-v` and `-q` win over `INK_LOG`; with a log channel of its own, naming an
area is more specific than a count, so `INK_LOG` and `--log` refine it. `-q` does not silence a
log that was asked for by name.

With no count, no filter, no file and no timings, nothing is installed and nothing costs anything.
With only `--log-file` or `INK_LOG_TIMINGS`, the floor is `info`.

## Output

**Human layer** (stderr, colour only when stderr is a terminal and `NO_COLOR`, `TERM=dumb` and
`Color::Never` do not forbid it; the same eight ANSI roles as the rest of `ink`):

```
   0.002s INFO  > command[build]
   0.003s INFO    > build[manual] build=0707...
   0.003s INFO      > stage[fetch] step=snapshot
   0.004s INFO      < stage[fetch] 1.1ms
   0.020s INFO      > stage[render] step=artifacts
   0.031s DEBUG       unit.planned outcome=rendered reason=unkeyed
   0.040s INFO      < stage[render] 20.0ms
   0.041s INFO      build.end status=succeeded took_us=38000
   0.041s INFO    < build[manual] 38.1ms
```

The tree is indentation by span depth, `>` opens, `<` closes with elapsed time and, when children
took part of it, the time of its own. `compact` is one line per event with the span path in front
and no span lines. Elapsed times are since the subscriber started, from the monotonic clock; the
layers do the timing, so the engine, the API and the watch read no clock for it.

**JSON lines** (`INK_LOG_FILE`, or `INK_LOG_FORMAT=json`): one object per line, numbers stay
numbers, an error keeps its causes:

```json
{"t":0.041,"level":"INFO","target":"uses_ink_build_local::local","name":"build.end",
 "fields":{"status":"succeeded","took_us":38000},"spans":[{"name":"command","fields":{"kind":"build"}}]}
```

Span lines carry `"span":"open"` or `"span":"close"` with `elapsed_us` and `own_us`.

**Timings** (`INK_LOG_TIMINGS=1`): a table when the run ends, one row per path in the span tree,
parents before children:

```
span                  count      total        own    slowest
command[build]            1     38.1ms      0.2ms     38.1ms
  build[manual]           1     37.9ms      0.4ms     37.9ms
    stage[render]         1     20.0ms     20.0ms     20.0ms
```

`own` is the time not spent in a child span. Under `--json` the table goes to the file as one
`{"timings": [...]}` line.

## Seam with the CLI

The CLI builds a `logging::Logging` from the environment (`Logging::env`), then from `-v`, `--json`,
`--log` and `--log-file`, and installs it once at the top of `run`; the guard it returns prints the
timing table when the command ends and holds the `command` span. That is the whole seam
(`cli/src/logging.rs`, two flags in `cli/src/grammar/args.rs`, one line in `Cli::run`). A failure to set
logging up is said on stderr as text and the run goes on without it; under `--json` stderr is a
machine channel, so it is not said there, and a log asked for with no file is withheld
(`Guard::withheld`).

## Cost

Disabled levels cost near nothing: the `tracing` macros check a static maximum level and a
callsite flag before evaluating any argument, and every conversion in this crate is an argument.
A test records at `info`, emits a thousand `trace` and `debug` events with a counting field, and
asserts the field was converted once, for the one `info` event. A counting allocator would need an
`unsafe impl`, which the layout check forbids, so "does not allocate" is argued from that: a
disabled event evaluates no argument, and the only work left is the level comparison.

A plugin call reads the clock only when its span is enabled.

## Testing

`logging::Capture` is a test subscriber that keeps spans and events as typed data: name, level,
target, typed fields, the span names around it. It records no time and starts no thread, so a
test is deterministic and sleeps for nothing. `capture.run(|| ...)` installs it for the current
thread, which covers `pollster` and a current-thread runtime; a server on its own thread runs its
loop inside `capture.run`.

The instrumentation is asserted on the key paths: an edge refusal is one `request.refused` event
with its status and cause chain, and the query of a request never reaches the log
(`edge/axum/tests/serve.rs`); a failed job logs its attempt and the chain of its error
(`control/tests/control/job.rs`); the supervisor's crash, restart and give-up sequence is asserted in `cli/src/selfhost/tasks.rs`. The crate's own tests cover redaction, the filter language, the
precedence of flags, laziness and the timing table.

## Not done, and why

- **Statement names.** `db::Statement` has a shape and no name, so a statement is logged by shape,
  rows and changes. Naming each typed query is a change to the db port.
- **Planner cause.** The reason is the lookup outcome and the key, not which input changed (see
  above).
- **Plugin slot and call name.** The wasm host's call point does not know which export it runs,
  so a `plugin` span is named by the plugin. Memory is not recorded: the host's limiter tracks a
  ceiling, not a high-water mark.
- **Edge reload.** `edge.reloaded` is named but nothing emits it: reloading a site's content is
  the preview's file watcher, in the CLI, which another change owns.
- **Source plugin internals.** A source plugin's `git` capability calls are not logged one by one;
  its whole call is a `plugin` span and its refusals are `plugin.refused`.
- **Log rotation.** `INK_LOG_FILE` appends; rotating it is the operator's.
