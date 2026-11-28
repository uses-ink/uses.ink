//! What a site takes in and gives out: intake of a built folder and export of a deploy.

mod domain;
mod export;
mod intake;
mod org;

pub use export::{Exports, Route};
