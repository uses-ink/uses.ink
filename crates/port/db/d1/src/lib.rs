//! The row port on Cloudflare D1 (decision 0097).
//!
//! [`D1`] implements [`db::Db`] over a D1 binding through workers-rs. It is built for
//! `wasm32-unknown-unknown` only: elsewhere this crate is empty, so the workspace builds natively.
//! The conformance suite of `db-conformance` runs on it inside workerd.

#![cfg(target_arch = "wasm32")]

mod cell;
mod d1;
mod error;

pub use d1::D1;
pub use error::{Error, Remote, Stage};
