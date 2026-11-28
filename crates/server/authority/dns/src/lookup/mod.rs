//! Asking a real nameserver: the message ([`Malformed`] is what a bad one is), the resolver that
//! sends it ([`System`] over [`Servers`]), the failures it has, and [`Chosen`], the one resolver a
//! server holds.

mod chosen;
mod failed;
mod system;
mod wire;

pub use chosen::Chosen;
pub use failed::{Failed, Unlisted};
pub use system::{Nameserver, Servers, System};
pub use wire::Malformed;
