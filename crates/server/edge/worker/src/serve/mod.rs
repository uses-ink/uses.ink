//! The serve path of the Worker: the edge's two ports over the control plane and a blob store, and
//! (on a Worker) the request in and the response out.
//!
//! The ports' futures must be `Send` and a binding's are not, so the adapters hold what they read
//! through in a `SendWrapper` and wrap the futures they return in one. A Worker's isolate runs one
//! thread, so the wrapper's thread check never fails there (decision 0200).

mod sited;
mod stored;

#[cfg(target_arch = "wasm32")]
mod fetch;

pub use sited::Sited;
pub use stored::{Cut, Opened, Reading, Stored};

#[cfg(target_arch = "wasm32")]
pub use fetch::{Answered, Serve};
