//! The model and its WIT mirror, converted both ways: the parts of `wit/mirror/`, written once
//! for both sides of the boundary, over this crate's bindings. Each part names the generated
//! types through `super::wit`.

use crate::bind::plugin::ink as wit;

include!("../../../../../wit/mirror/mod.rs");

pub use refusal::{Malformed, Unmirrored};
pub(crate) use snapshot::Seen;
