//! The mail port of the control plane (decision 0066): typed messages and a [`Mailer`] that
//! sends them.
//!
//! - [`Address`] is an email address checked where it is read and kept in lower case.
//! - A [`Message`] is a recipient and a [`Template`]. The template is an enum, so no call site
//!   formats text: its subject and its body are written once, by [`Template::subject`] and
//!   [`Display`](std::fmt::Display).
//! - A [`Link`] holds a secret (a one-time token in a URL). It prints as `Link(<redacted>)` in
//!   `Debug`, and its text is reached only through `Display`, which the adapters call to write
//!   the body.
//! - [`Mailer`] is the port. [`Capture`] keeps what it is given, for tests; [`Stdout`] writes
//!   each message to a writer, for a development run that has no mail server. The SMTP adapter is
//!   `mail/smtp`.

mod address;
mod capture;
mod link;
mod mailer;
mod message;
mod stdout;

pub use address::{Address, AddressError};
pub use capture::Capture;
pub use link::{Base, BaseError, Link};
pub use mailer::Mailer;
pub use message::{Message, Template};
pub use stdout::Stdout;
