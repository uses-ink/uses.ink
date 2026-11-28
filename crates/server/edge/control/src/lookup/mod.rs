//! The host-to-deploy lookup: the lookup itself ([`found`]), the half that holds the control plane
//! and places blobs as files ([`owner`]), the half the server calls ([`caller`]), what they keep
//! ([`cache`]) and how a lookup fails ([`error`]).

pub(crate) mod cache;
#[cfg(feature = "files")]
pub(crate) mod caller;
pub(crate) mod error;
pub(crate) mod found;
#[cfg(feature = "files")]
pub(crate) mod owner;
