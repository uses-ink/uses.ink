//! Where a git repository's branch is now, on a Worker (decision 0136).
//!
//! [`Smart`] is the watch's [`watch::Heads`] over a [`Fetch`]: it asks the forge for
//! `<repository>/info/refs?service=git-upload-pack`, reads the reference advertisement that
//! comes back and picks from it what `git ls-remote` would, and fetches no object. A Worker has no
//! sockets and no `git`, so this is the whole protocol the poll needs: one GET and a list of
//! packets. `watch/git` is the same read over `gix` for the self-host, and both are held to the
//! same answers (`tests/local.rs` reads one repository both ways).
//!
//! - [`Fetch`] is the HTTP call: [`Fetching`] (`wasm32` only) is the Worker's, [`Replay`] serves
//!   recorded bodies for a test.
//! - [`Advertisement`] is the parse, with no I/O.
//! - [`Case`] lists the suite over the three forges' recorded advertisements
//!   (`fixtures/`), which a native test and a Worker both run.

mod advert;
mod case;
mod error;
mod fetch;
mod replay;
mod smart;

#[cfg(target_arch = "wasm32")]
mod fetching;

pub use advert::{Advertisement, HeadOr};
pub use case::Case;
pub use error::{Absent, Misread, Unlisted};
pub use fetch::{Basic, Fetch, Get, Reply};
pub use replay::{Forge, Replay, Unserved};
pub use smart::Smart;

#[cfg(target_arch = "wasm32")]
pub use fetching::{Failed, Fetching};
