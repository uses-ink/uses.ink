//! The nonce store as a Durable Object: the state machine's wire, the object's handler and the
//! client a Worker reaches it with.

mod error;
#[cfg(target_arch = "wasm32")]
mod remote;
mod serving;
mod wire;

pub use error::Malformed;
#[cfg(target_arch = "wasm32")]
pub use remote::{Remote, Unreached};
pub use serving::Serving;
pub use wire::{Answer, Wire};
