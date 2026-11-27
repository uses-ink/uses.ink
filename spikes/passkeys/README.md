# Spike: webauthn-rs-core on wasm32-unknown-unknown

Auth slice 2 (decision 0068, `docs/auth.md` section 22, spike 1). Question: does
`webauthn-rs-core` build for `wasm32-unknown-unknown`, which the Worker of M4 needs?

**Verdict: it compiles and links.** `cargo build --release --target wasm32-unknown-unknown` of a
cdylib that builds a registration challenge (`src/lib.rs`) passes on the farm. Two crates need
their JavaScript entropy switch on that target: `getrandom` 0.4 with `wasm_js` (under `rand` 0.10,
which draws the challenge) and `uuid` with `js`. Both are in `Cargo.toml` here.

What this does **not** show:

- It was **not executed** in `workerd` or any other wasm runtime. Only the build is proven. The
  verifier's behaviour is exercised natively by the tests of `edge/api` (`tests/passkey.rs`); the
  assumption that the same code behaves the same on wasm32 rests on the tree being pure Rust.
- The `.wasm` size was not measured: the farm host keeps this crate's `target/`.
- No browser was involved (`docs/auth.md` section 22, spike 2 stays open).

## What the tree is

Read with `cargo tree -e normal --target wasm32-unknown-unknown` on 2026-11-27:

| | |
|---|---|
| `webauthn-rs-core` | 0.6.2, MPL-2.0, released 2026-10-31 |
| Unique crates | 167 (cdylib and all) |
| Resolves | `rsa` 0.10.0 (RUSTSEC-2023-0071 is patched from 0.10), `crypto-glue` 0.2.3, `rand` 0.10.4, `getrandom` 0.4.3, `serde_cbor_2` 0.13.0, `x509-parser` 0.18.1 |
| Licences (unique crates) | MIT OR Apache-2.0 for nearly all; Unicode-3.0 (19); BSD-3-Clause is `subtle` (already excepted); `memchr` Unlicense OR MIT; `zerocopy` BSD-2-Clause OR Apache-2.0 OR MIT; `flagset` Apache-2.0 |
| **MPL-2.0** | exactly four: `webauthn-rs-core`, `webauthn-rs-proto`, `webauthn-attestation-ca`, `crypto-glue`. `base64urlsafedata` is not a separate crate in 0.6.2, so the list in `docs/auth.md` section 15 had one name too many |

`deny.toml` therefore needs four crate-scoped exceptions (`MPL-2.0`), decided in the build note of
0068. The final `farm check` runs `cargo deny` on the whole graph, which is the proof that the list
is complete natively.

## The library, as read

- `WebauthnCore::new_unsafe_experts_only` is the only constructor. Its documentation says the
  signature may change within minor versions, so the workspace pins `=0.6.2`.
- It hard-sets `require_valid_counter_value: true`: any counter that does not rise, once either
  side is nonzero, is `CredentialPossibleCompromise`. Section 4's counter policy (flag a synced
  passkey, refuse only a hardware one) cannot be set through it, so the adapter hands the library
  a stored credential with counter 0 and applies the policy itself.
- Discoverable sign-in is possible with an empty credential list and
  `authenticate_single_credential`, which ignores the state's credential list.
