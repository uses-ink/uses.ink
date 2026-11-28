//! What the tests build sites from: an in-memory snapshot, a stand-in format that reads a few
//! lines, a stand-in theme that writes a few tags, and pins held in a list.

mod images;
pub(super) mod lines;
mod pins;
mod reads;
mod theme;
mod tree;

pub use images::{Stand, Store};
pub use pins::Pinned;
pub use reads::{codes, on, path, settings, site, subject};
pub use theme::Plain;
pub use tree::Tree;
