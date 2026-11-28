//! The Worker shell of the edge (decisions 0135 and 0200).
//!
//! The serve path is [`Sited`] and [`Stored`], the edge's two ports over the control plane and a
//! blob store, and [`Serve`], which turns a Worker's request into the edge's and its answer into a
//! Worker's response. The nonce store as a Durable Object has three parts, below.
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

mod api;
mod harness;
mod nonce;
mod schema;
mod serve;

#[cfg(target_arch = "wasm32")]
mod platform;

pub use api::{Gate, Hosting, Pending, Pinned, Unready, Vars, Wired};
pub use harness::Hosted;
pub use nonce::{Answer, Malformed, Serving, Wire};
pub use schema::Migrations;
pub use serve::{Cut, Opened, Reading, Sited, Stored};

#[cfg(target_arch = "wasm32")]
pub use api::Calls;
#[cfg(target_arch = "wasm32")]
pub use nonce::{Remote, Unreached};
#[cfg(target_arch = "wasm32")]
pub use platform::Platform;
#[cfg(target_arch = "wasm32")]
pub use serve::{Answered, Serve};
