//! Frontmatter values: what a format hands over ([`Raw`]), how a value type reads it
//! ([`FromFront`]), and what a refused value wanted ([`Expected`]).
//!
//! The format parses its frontmatter syntax (YAML for markdown) into [`Raw`] and reports what
//! does not parse; the value types here say what they accept, once, for every format.

mod raw;
mod read;
mod refusal;

pub use raw::Raw;
pub use read::FromFront;
pub use refusal::{Cause, Expected, Form, Refusal, Wanted};
