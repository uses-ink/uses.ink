# uses-ink-authority-email

The email authority, `ink:authority-email` (decision 0094): the one-time link of decision 0066, used
as a proof that someone reads mail at an address. It makes no session and touches no account. An
invitation to an organization (`docs/auth.md` section 12) and a recovery step are the callers.

## What it may use

`uses-ink-control` (which carries the sign-in rows) and `uses-ink-mail`. Its tests run the sign-in
rows over the SQLite adapter and read what was mailed from the `Capture` mailer. The full list is in
[the crate index](../../../../docs/crates.md).

## Where things are

- `crates/server/authority/email/src/email.rs` is `Email`: `send` stores a proof and mails its link,
  `confirm` spends it and returns the address.
- `crates/server/authority/email/src/error.rs` is what either call fails with.
- `crates/server/control/signin/src/store.rs` holds `SignIn::vouch` and `SignIn::attest`, the rows.

## How to test it

    farm cargo nextest run -p uses-ink-authority-email
