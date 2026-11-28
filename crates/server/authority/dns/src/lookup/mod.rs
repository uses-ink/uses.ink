//! Asking a real nameserver: the message ([`Malformed`] is what a bad one is), the resolver that
//! sends it ([`System`] over [`Servers`]), the failures it has, and [`Chosen`], the one resolver a
//! server holds.

mod chosen;
mod failed;
#[cfg(feature = "system")]
mod system;
#[cfg_attr(
    not(feature = "system"),
    expect(dead_code, reason = "the message is built and read only by the system resolver")
)]
mod wire;

pub use chosen::Chosen;
pub use failed::{Failed, Unlisted};
#[cfg(feature = "system")]
pub use system::{Nameserver, Servers, System};
pub use wire::Malformed;
