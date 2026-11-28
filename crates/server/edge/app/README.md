# uses-ink-edge-app

The product Worker: one `fetch` that answers the signed API on `API_HOST` and serves sites on every
other host, over D1, R2, Queues, a secret binding and the nonce Durable Object (decision 0201).

## What it answers

- `API_HOST`: the routes of `edge-api` under `/v1` (enrol, keys, sites, deploys, intake, the
  webhooks of forges), signed with `Ink-Sig-1` as `ink` signs them.
- Any other host: the serve path of the edge, the same as `ink serve`.
- What it does not have yet answers `501` with the code `worker.route.pending` and its fix: the
  dashboard, sign-in by link, passkeys and builds.

## What a deploy needs

Nothing here is deployed by the build or the tests. For the owner, in order:

1. A Cloudflare account, and `wrangler login` or `CLOUDFLARE_API_TOKEN` in the environment.
2. `wrangler d1 create uses-ink`, and its id in `wrangler.toml` (`database_id`).
3. `wrangler r2 bucket create uses-ink-blobs` and `wrangler queues create uses-ink-wake`.
4. `cargo xtask worker migrations` writes the migrations to `target/hosted/app/migrations`, then
   `wrangler d1 migrations apply DB --remote` in this folder applies them.
5. `wrangler secret put SEALING_KEY`: 64 hexadecimal digits, from a source of randomness
   (`openssl rand -hex 32`). A lost key makes the sealed credentials unreadable; keep a copy.
6. `ORIGIN`, `API_HOST` and `OPERATOR_KEYS` in `wrangler.toml` (`[vars]`). The operator's line is
   made by `ink login --data <folder>`, which prints nothing secret: its public half is the line
   `<folder>/operator-keys` gets.
7. `wrangler deploy` here, which builds with `worker-build --release`.
8. Routes: `API_HOST` and `*.<host of ORIGIN>` to the Worker, in the zone's dashboard or with
   `routes` in `wrangler.toml`. Custom domains of sites need Cloudflare for SaaS, which is not in
   this Worker.

## Run it locally

`cargo xtask worker app` builds the Worker, applies the migrations to a local D1, starts
`wrangler dev --local` and runs `ink` against it: enrol a key, create a site, push a tree, fetch it,
refuse a replay, suspend the site and see the `403`. Nothing leaves the machine.

## Limits of a Worker

D1 holds integers to 2^53 and takes 100 bound parameters a statement, R2 needs a known length or a
multipart upload, and a request is held in memory, so the upload limits default below the
self-host's. Each is a variable.
