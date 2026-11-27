## 0007 WIT is the plugin boundary; builder-side plugins are components in v1

Accepted. Plugin boundary types are written in WIT (`ink:*@0.1`). Source, format and theme
plugins are wasm components run by wasmtime in the builder from v1. Authority plugins are native
crates against the WIT-generated types, because the edge is a Worker and cannot host wasmtime.
**Why:** follows from 0015; once the host exists, every builder plugin goes through it, which
proves there is no back door. **Reversal:** if component overhead on formats is measured above
20% of build time, format plugins become native crates against the same types until it is fixed.
