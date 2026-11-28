//! The API on the Worker (decision 0201): the signed routes under `/v1`, the webhooks, and the
//! routes the Worker does not have yet, answered as such.
//!
//! [`Hosting`] reads what the Worker's variables say and wires [`Wired`] over the ports; nothing
//! here names D1, R2 or a Durable Object, so the same value is tested over SQLite and a directory.

#[cfg(target_arch = "wasm32")]
mod call;
mod hosting;
mod pending;
mod pinned;

#[cfg(target_arch = "wasm32")]
pub use call::Calls;
pub use hosting::{Hosting, Unready, Vars, Wired};
pub use pending::{Gate, Pending};
pub use pinned::Pinned;
