---
number: "0134"
title: "The Cloudflare mail adapter calls its provider through a typed Transport, and the mail port gets a conformance suite"
status: built
date: 2026-11-28
area: hosting
supersedes: []
---

## 0134 The Cloudflare mail adapter calls its provider through a typed Transport, and the mail port gets a conformance suite

Built (2026-11-28), A4 of `docs/hosted.md`.

- **`crates/port/mail/cf`** (`uses-ink-mail-cf`): `Cf<T: Transport>` writes a `Request` (sender, one recipient, subject,
  plain-text body) from a `Message` and hands it to `T`. The provider is still open (HQ9), so no endpoint or credential
  is in the crate: a real transport, when HQ9 closes, is one `impl Transport` that knows the URL and the key. `Fake` is the
  transport for tests and the rig; it keeps requests and can be told to refuse. The crate has no `wasm32`-only code.
- **Failures** are `Error { to, source }`: the recipient and the transport's own typed error. The body holds a link's
  token, so `Request` prints it as a byte count, and the suite checks that a failure never shows the token.
- **`crates/port/mail/conformance`** (`uses-ink-mail-conformance`): a `Harness` trait (the mailer, what it delivered as
  `Delivered { to, subject, body }`, and a way to make it fail) and four cases: a message arrives with the whole link and
  its lifetime, templates keep their order and subjects, an address that could carry a header is refused where it is read,
  a failure is typed and delivers nothing. A mailer cannot be read back through the port, so each adapter's test says how.
  `Capture` (typed messages) and `Stdout` (parsed text, and a writer that can fail) pass it in the conformance crate's
  tests, `Cf` over `Fake` in its own tests and in workerd.
- **`Case`** lists the cases with names, as in the other suites, so the rig can run them without `pollster`.

Numbers: native, 20 tests pass across the mail crates (4 cases for each of 3 mailers, plus the units).
**Not done:** SMTP is not in the suite (it needs a server; its tests stay at composing the message); the real provider call.
**Reversal:** if HQ9 picks a provider with a Workers binding instead of HTTP, `Transport` takes the binding and nothing else moves.
