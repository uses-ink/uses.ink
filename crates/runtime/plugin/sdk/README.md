# uses-ink-sdk

The SDK is the guest side of the plugin boundary (decision 0035): what a plugin built as a wasm
component links to reach its host and to speak the model. A plugin is a native core, written
against the model's types and traits, plus a thin guest crate that exports it through the SDK.

## What it may use

Only `freehand` and `uses-ink-model`. It is MIT OR Apache-2.0 like the WIT packages in `wit/`,
because a third party links it into their own plugin. The list for every crate is in
[the crate index](../../../../docs/crates.md).

## Where things are

- `bind` in `crates/runtime/plugin/sdk/src/lib.rs` is the bindings `wit-bindgen` writes for
  `ink:host/plugin`, every host capability and every model type in WIT. A guest maps its world's
  imports onto these, so one set of types exists per component.
- The conversions between those types and the model are written once, in `wit/mirror/`, for both
  sides, and included by `crates/runtime/plugin/sdk/src/mirror.rs`. `Unmirrored` and `Malformed` are
  their refusals.
- `Served` in `crates/runtime/plugin/sdk/src/format.rs` serves a format's `ink:format` exports
  from its native core through the `Format` trait.
- `Host` in `crates/runtime/plugin/sdk/src/host.rs` is the host's capabilities as the native
  traits a core is written against: `Highlight`, `Sink` and `Git`.
- The guests that use it are `crates/runtime/plugin/format/markdown/guest`,
  `crates/runtime/plugin/format/obsidian/guest`, `crates/runtime/plugin/theme/quiet/guest` and
  `crates/runtime/plugin/source/git/guest`.

## How to test it

The SDK has no tests of its own: it is code for a wasm target, and a bug in it shows as a component
that disagrees with its native core. Those comparisons are the tests of
`crates/runtime/host/wasmtime/tests/`, which load each guest's component and run it against the core
it wraps. Build the components and run them on the farm with `farm check test`; the `wasm` job of
`.config/farm.toml` (`farm check wasm`) covers the wasm side.
