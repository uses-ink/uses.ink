## 0015 Themes are wasm components in v1

Accepted, owner. Themes are WebAssembly components from the first release, written in any
language that targets components; first-party themes are Rust. Template overrides are something
a theme offers through the `asset.override` capability. **Why:** third-party themes on day one,
sandboxed, with the same contract first-party themes use.
