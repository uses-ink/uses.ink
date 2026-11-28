//! The blob port on Cloudflare R2 (decision 0131).
//!
//! [`R2`] implements [`blob::Blob`] over an R2 bucket binding through workers-rs. It is built for
//! `wasm32-unknown-unknown` only: elsewhere this crate is empty, so the workspace builds natively.
//! The conformance suite of `blob-conformance` runs on it inside workerd.
//!
//! A write hashes the bytes as it takes them and holds at most one part of them. R2 needs a known
//! length or a multipart upload, so a blob that fits one part is put whole after its hash matched,
//! and a larger one is cut into parts of [`Limits::part`] bytes as it grows. A hash that does not
//! match aborts the upload, so nothing is stored.

#![cfg(target_arch = "wasm32")]

mod error;
mod limits;
mod r2;
mod reading;
mod writing;

pub use error::{Call, Error, Reason};
pub use limits::{Limits, Refused};
pub use r2::R2;
pub use reading::Reading;
