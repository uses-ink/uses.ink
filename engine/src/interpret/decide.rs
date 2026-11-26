//! How a decision is made: the [`Ladder`] mechanism, the witnesses every ladder is built from,
//! each code's precedence as one type alias, and each code's own words.
//!
//! [`Ladder`]: ladder::Ladder

mod explain;
mod ground;
pub(crate) mod ladder;
pub(crate) mod precedence;
pub(crate) mod witness;

pub(crate) use explain::Explain;
pub(crate) use ground::{Filed, Sourced};
