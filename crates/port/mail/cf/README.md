# uses-ink-mail-cf

The mail port (`uses-ink-mail`) on Cloudflare (decision 0134). `Cf<T>` writes a `Request` (sender, one
recipient, subject, plain-text body) from a `Message` and gives it to a `Transport`. The provider is
undecided (`docs/hosted.md` HQ9), so no endpoint or credential lives here: a transport is the one place
that will know them, and `Fake` is the transport the tests and the rig use (it keeps requests and can be
told to refuse).

- A failure is `Error { to, source }`: the recipient is named and the body, which holds a link's token, is
  not. `Request` prints its body as a byte count.
- The crate builds natively and on `wasm32-unknown-unknown`; nothing in it needs the platform.
- `uses-ink-mail-conformance` is the port's suite: delivery with the whole link, template order, addresses
  that could carry a header, a typed failure that delivers nothing. `Capture` and `Stdout` pass it in
  `mail-conformance/tests`, this adapter in its own tests and in workerd (`cargo xtask worker mail`).
