## 0004 Hosted runtime: Worker edge, Container builds

Accepted. A workers-rs Worker serves, runs the control plane and takes webhooks; every build runs
`ink build` in a Cloudflare Container started by a per-handle Durable Object. **Why:** a 128 MB
isolate cannot clone repos with history or render large vaults; one build path for every source.
**Reversal:** container cold starts above about 15 s push-to-live: add a wasm fast path for small
snapshots. Price or availability changes: the same binary runs on a VPS behind the same queue.
