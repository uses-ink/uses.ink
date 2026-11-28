# uses-ink-edge

The edge serves a deploy. An `Edge` turns a `Request` (a method, a target and header lines as
bytes) into a typed `Reply`: a `Response` and the `Outcome` that explains it. It resolves the host
to a deploy, opens the blob behind a route, and refuses what it should not serve as a `Refusal`
that carries its data. It owns no socket, file, clock or runtime, and nothing in it panics on any
input.

## What it may use

Only `uses-ink-model` and `uses-ink-logging` (decision 0051). It reaches deploys and blobs through
two ports, `Sites` and `Blobs`, defined in `crates/server/edge/src/serve/port.rs`. The adapters are
separate crates, and none of them takes another: `crates/server/edge/axum` (sockets, time budgets,
shutdown), `crates/server/edge/folder` (the folders `ink build` writes),
`crates/server/edge/control` (a site's live deploy from the control plane) and
`crates/server/edge/api` (the API and intake). The list for every crate is in
[the crate index](../../../docs/crates.md).

An adapter for the hosted edge is planned and is not built.

## Where things are

- `crates/server/edge/src/http/` is the HTTP vocabulary as small types: request, response,
  ranges, conditions, etags, encodings and refusals.
- `crates/server/edge/src/serve/` is `Edge` and its ports, the hostname and target types, and the
  error.
- Every response carries `nosniff`, a CSP and the cross-origin policy. A manifest entry's own
  headers win over those, and `Strict-Transport-Security` is added over TLS.

## How to test it

`crates/server/edge/tests/` holds `serve.rs` (the behaviour), `hostile.rs` (hostile input) and
`property.rs` (properties over request shapes), with shared setup in
`crates/server/edge/tests/support/`. On the farm:

    farm cargo nextest run -p uses-ink-edge

The socket tests of the axum adapter run a server on a thread with short time budgets; see
`.config/nextest.toml` for the group that keeps them from starving each other.
