//! The conformance suite of the edge's `Sites` and `Blobs` ports.
//!
//! Each [`Case`] is an async function over a [`Harness`], so a Worker's event loop can drive it as
//! well as `pollster`. [`suite!`] makes every case a native test. A case that the adapter cannot
//! offer says so as [`Outcome::NotOffered`], which a runner reports as such and never as a pass.
//!
//! ```ignore
//! edge_conformance::suite!(|| MyHarness::fresh());
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::indexing_slicing,
    clippy::similar_names,
    clippy::manual_assert,
    reason = "a conformance case stops at the first surprise"
)]

mod case;
mod harness;
mod seed;
mod suite;

pub use case::{Case, Outcome};
pub use harness::{Harness, Offers, Unseeded};
#[doc(hidden)]
pub use pollster;
pub use seed::{Page, Seed};
