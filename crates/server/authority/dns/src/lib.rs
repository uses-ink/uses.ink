//! The DNS authority, `ink:authority-dns` (decision 0093, `docs/architecture.md` section 12): a
//! site proves it controls a name by publishing a TXT record at `_uses-ink.<name>` that carries
//! the token of its claim.
//!
//! - [`Resolver`] is the port: it reads the TXT records of a [`Host`] and nothing else. [`Fake`]
//!   answers from a table for tests, [`Offline`] answers that there is none (a self-host that has
//!   not been given one); [`System`] asks real nameservers over UDP and TCP within a limit, and
//!   [`Chosen`] is the one concrete type a server holds, whichever the operator picked.
//! - [`Record`] is the one spelling of what a record says, written for a person to publish
//!   ([`Challenge`]) and read back from what a lookup found.
//! - [`Dns`] runs the proof: it reads the records, hands what it found to the control plane
//!   ([`control::Control::prove_domain`]) and returns the name that now serves the site. The
//!   control plane decides, so a token that is another site's claim proves nothing here.

mod dns;
mod error;
mod fake;
mod host;
mod lookup;
mod record;
mod resolver;

pub use dns::Dns;
pub use error::Error;
pub use fake::{Fake, NoResolver, Offline, Unreachable};
pub use host::Host;
pub use lookup::{Chosen, Failed, Malformed, Unlisted};
#[cfg(feature = "system")]
pub use lookup::{Nameserver, Servers, System};
pub use record::{Challenge, Record};
pub use resolver::Resolver;
