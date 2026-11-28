# uses-ink-host-wasmtime

This crate runs plugins as wasm components. A `Runtime` is one wasmtime engine with fuel and epochs
turned on, a `Budget` for every call, an optional `Cache` of compiled components and the host's
capabilities, implemented once and linked into every component. `Runtime` loads a format or a theme
as a `Component`, which implements the engine's own `Format` or `Theme` trait, so the engine runs a
component the way it runs a native plugin (decisions 0007, 0015 and 0035).

## What it may use

`freehand`, `uses-ink-model`, `uses-ink-engine`, `uses-ink-host-native`, `uses-ink-host-image`,
`uses-ink-logging`, and the native cores of the plugins it is tested against (the markdown and
obsidian formats, the quiet theme and the git source). wasmtime is seen only here: the conversions
between its generated types and the model are written once in `wit/mirror/` and shared with the
guests' SDK. The list for every crate is in [the crate index](../../../../docs/crates.md).

## Where things are

- `Runtime` is in `crates/runtime/host/wasmtime/src/runtime.rs` and `Component` in
  `crates/runtime/host/wasmtime/src/component.rs`.
- Budgets are `crates/runtime/host/wasmtime/src/budget.rs`. Running out of fuel, time or memory, a
  trap, or a value the model refuses is a typed `Failure` that becomes a diagnostic, never a
  panic. Every format call gets a fresh instance, and a theme's instance is replaced after a
  failed call.
- The host's capabilities are `crates/runtime/host/wasmtime/src/capability/`, and the compiled
  cache is `crates/runtime/host/wasmtime/src/cache.rs`. Loading a `.cwasm` from the cache is one
  of the two places unsafe code is allowed (decision 0035; the list is in `.config/layout.toml`).
- `crates/runtime/host/wasmtime/hostile` is a format that misbehaves on purpose, so the budgets
  are tested against a real component.

## How to test it

The tests in `crates/runtime/host/wasmtime/tests/` run the components against their native cores
and the hostile one against the budgets. They need the components that `cargo xtask component`
writes to `target/plugin/`, so run the gate's stage that builds them first:

    farm check test

For one test binary after that, `farm cargo nextest run -p uses-ink-host-wasmtime`. The timing
tests need a quiet core, which is why the farm places them.
