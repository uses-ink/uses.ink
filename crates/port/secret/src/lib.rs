//! The secret port: credentials sealed at rest (decision 0031).
//!
//! A credential is sealed with the id of the row that holds it as associated data
//! ([`Binding`]), so a sealed value copied to another row fails to open. The key never crosses
//! this port: an adapter seals and opens, and nothing here can ask it for the key.
//!
//! A [`Plain`] credential redacts itself in `Debug`, has no `Display`, no `Clone` and no serde
//! impl, and is zeroed when dropped, so it reaches no log by accident.
//!
//! The adapters are `secret/local` and, hosted, a Workers secret; the suite every adapter runs
//! is `secret/conformance`.

mod error;
mod plain;
mod port;

pub use error::Error;
pub use plain::{Binding, Plain, Sealed};
pub use port::Secret;
