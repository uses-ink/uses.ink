---
number: "0079"
title: "Hosted is adapters of the existing ports: queue rows on D1, blobs through the egress gateway, nonces in a Durable Object"
status: proposed
date: 2026-11-28
area: hosting
supersedes: []
---

## 0079 Hosted is adapters of the existing ports: queue rows on D1, blobs through the egress gateway, nonces in a Durable Object

Proposed (2026-11-28); the owner has not read it. `docs/hosted.md` is the plan for M4 and holds
the packages, the order and the owner questions. The choices to push back on:

- **The queue is rows on D1, Cloudflare Queues only wakes a consumer.** Cloudflare Queues has no dedupe
  key, and its consumer is a push batch handler; a pull consumer's visibility timeout is the HTTP
  API, which a Worker does not call. The `Queue` port needs a `Duplicate` answer on `send` and a
  `receive` with a lease inside the Worker.
  `crates/port/queue/rows` is already written over any `db::Db` and runs on D1, so hosted uses it over
  `crates/port/db/d1`, and `crates/port/queue/cf` sends one wake-up message after a `send`. The rows stay the truth.
- **A container reaches R2 through the Worker.** The container has no R2 binding and holds no
  credential. Its builder `PUT`s each blob to a name the egress handler owns (0037); the handler
  checks the hash and writes the object under the tenant in its props.
- **The signed-request nonce store is a trait, held per account in a Durable Object when hosted**
  (`docs/auth.md` section 5, "Hosted"), with a boot rule on every construction. A D1 unique key
  passes the same suite, so the choice can be reversed.
- **The Worker is a twin of the edge.** The `Send + Sync` edge traits get a local flavour behind one
  blanket impl (the reversal written in 0051); no logic changes.
- **Worker crates are `cfg(target_arch = "wasm32")`-gated workspace members.** The native stages run
  with `--workspace` and see an empty crate; a `worker` stage builds them for
  `wasm32-unknown-unknown` and runs the conformance suites in `workerd`. Excluding them like
  `spikes/` is the alternative; the first package settles it. Conformance suites expose their cases as async functions for it; `suite!`
  is unchanged.
- **No M4 crate is added before the layout freeze (S6 of `docs/layout.md`).** Until then M4 is
  decisions, docs and spikes under `spikes/`.

**Not decided here:** the account, domains, billing, bucket jurisdiction, mail provider, brand list,
budget numbers and legal contact. They are `docs/hosted.md` section 12, each with a default.
**Why:** the ports were shaped for D1 and R2 (0030, 0051, 0052), so the hosted shape is adapters,
and the first risk is the rig that runs their suites without an account. **Reversal:** a spike
shows D1's integer boundary or the Worker's bundle size cannot hold the plan: split the Worker in
two, or keep rows in a Durable Object's SQLite storage behind the same `Db` port.
