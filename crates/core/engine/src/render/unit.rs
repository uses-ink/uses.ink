//! The render unit index (decision 0048): which pages a render makes again and which it takes
//! from a [`Cache`](crate::cache::Cache). A page is one unit. Its key is the hash of everything the theme may read
//! to write it: the engine's release, the theme's own identity, where the site is served, the
//! page, and the part of the site the theme says it reads ([`Reads`]). A key never holds a
//! time, so a build that changes nothing a page reads takes the page from the cache, and a
//! build with a cache gives the bytes a build without one gives.

mod key;
mod plan;

pub(crate) use key::Units;
pub(crate) use plan::Reason;
pub use plan::{Host, Outcome, Reads, Step, Unit};
