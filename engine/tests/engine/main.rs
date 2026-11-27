//! The engine through its public API: each inferrer's precedence, links, routes, all or nothing,
//! rendering, determinism, and the site fixtures.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_engine as engine;

mod change;
mod embed;
mod incremental;
mod interpret;
mod link;
mod picture;
mod render;
mod sites;
mod support;
