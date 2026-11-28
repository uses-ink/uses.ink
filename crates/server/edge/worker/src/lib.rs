//! The Worker shell of the edge (decision 0135).
//!
//! The nonce store as a Durable Object has three parts. The state machine is
//! `uses-ink-edge-object-nonce`'s `Ledger`. [`Serving`] puts it behind the wire: it takes a claim as
//! JSON bytes and answers a verdict as JSON bytes, which is all the object's `fetch` has to do, and
//! it is also an [`edge_api::Nonces`] store, so the suite runs on it natively. [`Remote`] is the
//! store a Worker uses: it reaches the object of a key through a namespace binding and implements
//! the same trait.
//!
//! The object's class is declared by the Worker that deploys it (`#[durable_object]` in a
//! dependency's code is not linked into the final module reliably), as a few lines around
//! [`Serving::handle`]. [`Remote`] is `wasm32` only; the rest builds natively.

mod error;
mod serving;
mod wire;

#[cfg(target_arch = "wasm32")]
mod remote;

pub use error::Malformed;
pub use serving::Serving;
pub use wire::{Answer, Wire};

#[cfg(target_arch = "wasm32")]
pub use remote::{Remote, Unreached};
