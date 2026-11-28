---
number: "0072"
title: "Git over https: one TLS stack, rustls over ring, and one licence exception"
status: accepted
date: 2026-11-27
area: security
supersedes: []
---

## 0072 Git over https: one TLS stack, rustls over ring, and one licence exception

Accepted (2026-11-27). The owner took the recommendation to fix what 0067 refused: the watch's
head read (`crates/server/watch/git`) and the source fetch (`crates/runtime/host/native`) now reach `https` remotes.
`Refused::Transport` and `Unlisted::Transport` are no longer produced for https; the model keeps
the `source.git.transport` problem for a scheme a later build lacks.

- **One TLS stack.** `gix` has three HTTP backends: `curl` (with OpenSSL or its own rustls) and
  `reqwest`, whose `blocking-http-transport-reqwest-rust-tls` feature turns on reqwest's `rustls`
  and with it `aws-lc-rs`, a second crypto provider and a C build. The tree already had rustls
  over `ring` for `lettre` (0066), so gix gets the TLS-free `blocking-http-transport-reqwest` and
  the workspace names `reqwest` with `rustls-no-provider`. The client is then built by us, in the
  new `https` crate: `rustls::ClientConfig` with the `ring` provider named explicitly (no process
  default is installed or read), the `webpki-roots` roots `lettre` uses plus any the operator adds,
  handed to reqwest with `tls_backend_preconfigured`. `Cargo.lock` has `ring` and no `aws-lc-rs`,
  `openssl`, `native-tls` or `curl`; `cargo tree -d` lists no TLS crate twice.
- **Roots.** Bundled web PKI roots, not the machine's store: the same trust on every host, nothing
  read from the environment. A forge with a private authority is trusted with `--git-ca <pem>`
  (repeatable), which adds trust for that chain and turns no check off. There is no flag or
  setting that disables verification.
- **Licence exceptions.** Only `webpki-root-certs` (CDLA-Permissive-2.0) is new in `deny.toml`:
  reqwest's `rustls-no-provider` feature also compiles `rustls-platform-verifier`, which depends
  on it, though nothing calls it. `ring` (ISC), `rustls-webpki`, `untrusted` and `webpki-roots`
  were already excepted per crate by 0066, and `rcgen` (dev only) is MIT OR Apache-2.0.
  `cargo deny check licenses advisories bans sources` passes.
- **Settings, all typed (`https::Https`, flags in `ink serve`).** `connect` (`--git-connect`,
  10 s), `total` for one request including its body (`--git-request`, 120 s; reqwest's blocking
  client would otherwise cut every request at 30 s), `follow` (`--git-redirects`: `never`,
  `first` (default, as git), `always`), extra roots (`--git-ca`). The scheme list stays
  `--git-scheme` (default `https`).
- **Redirects.** `gix` installs its own redirect policy after ours and it is not configurable: a
  redirect that is not http to https or the same scheme is never followed (so https to http
  stops), a change of host must keep the request's tail, and a chain ends after 50 hops. Our client
  is also `https_only`. What is a setting is whether any are followed. A hop count smaller than 50
  would need a patch to `gix-transport`; not done.
- **Credentials** still come through the `secret` port to gix's credential callback, as before;
  nothing new reads or logs them.
- **Proxy.** None. `gix`'s reqwest backend ignores `http.proxy`, the client is built with
  `no_proxy` and reqwest's system-proxy feature is off, so `HTTPS_PROXY` has no effect. A
  self-host behind a mandatory proxy needs a decision of its own.
- **Not done: a response size budget.** gix's reqwest backend offers no hook on the response body.
  A clone is cut by the existing byte budget while the pack arrives, and every request by `total`;
  a reference list has no size limit of its own.
- **Tests (no network).** A TLS server on the loopback with an `rcgen` authority, answering
  `git upload-pack`: a head read over https with a trusted root, an untrusted certificate that
  fails with the TLS error among its causes and receives no request, a redirect to plain http that
  never reaches its listener, a server that never answers cut at `total`, and a clone over https
  that equals the one over `file`.

**Why:** self-hosting that cannot reach GitHub or a Forgejo is not self-hosting, and a second TLS
stack is a second thing to audit and patch. **Reversal:** if `gix-transport` gains a redirect
limit or a body limit, set them in `https::Https`; if the platform verifier is ever wanted, the
roots become a setting and `webpki-root-certs` is already allowed.
