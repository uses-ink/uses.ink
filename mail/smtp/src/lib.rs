//! The SMTP adapter of the mail port (decision 0066), on `lettre` with rustls.
//!
//! [`Settings`] holds everything a connection needs: host, port, how the connection is secured
//! ([`Security`]), an optional [`Login`] and the [`From`](Settings::from) address. The password
//! is a [`secret::Plain`], so it prints as `Plain(<redacted>)`, is zeroed when dropped and
//! appears in no error and no log. [`Smtp::compose`] builds the MIME message without sending, which is
//! what the tests check; sending needs a server.

mod error;
mod smtp;

pub use error::Error;
pub use smtp::{Login, Security, Settings, Smtp};
