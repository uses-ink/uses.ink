//! What the control plane fails with, by family: what it refuses, what it cannot read back and what
//! the ports failed with.

mod ports;
mod refusal;
mod unreadable;

pub use ports::{Error, Failed};
pub use refusal::Refusal;
pub use unreadable::Unreadable;
