//! The host-to-deploy lookup: the half that holds the control plane ([`owner`]), the half the
//! server calls ([`caller`]), what they keep ([`cache`]) and how a lookup fails ([`error`]).

pub(crate) mod cache;
pub(crate) mod caller;
pub(crate) mod error;
pub(crate) mod owner;
