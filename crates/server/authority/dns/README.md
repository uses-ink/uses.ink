# uses-ink-authority-dns

The DNS authority, `ink:authority-dns` (decision 0093): a site proves it controls a custom domain by
publishing a TXT record at `_uses-ink.<name>` that carries the token of its claim. This crate reads
the record; the control plane holds the claim and decides what the record proves.

## What it may use

`uses-ink-control` and nothing else in production: the `Resolver` port is defined here, and the
adapter that asks a real DNS server is hosted work (`docs/hosted.md` L2). Its tests run the control
plane over the SQLite and directory adapters with a `Fake` resolver. The full list is in
[the crate index](../../../../docs/crates.md).

## Where things are

- `crates/server/authority/dns/src/resolver.rs` is the `Resolver` port; `fake.rs` is the `Fake`
  that answers from a table and can be told to fail.
- `crates/server/authority/dns/src/record.rs` is the one spelling of a proof record and the
  `Challenge` a person is told to publish.
- `crates/server/authority/dns/src/dns.rs` is `Dns`, which reads the records and hands them to
  `control::Control::prove_domain`.

## How to test it

    farm cargo nextest run -p uses-ink-authority-dns
