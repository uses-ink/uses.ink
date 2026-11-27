//! The model through its public API: typed codes and their erased forms, frontmatter values,
//! and the walk over a document.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_model as model;

mod code;
mod coded;
mod front;
mod record;
mod walk;
