//! The secret port on a Worker (decision 0133).
//!
//! [`Worker`] seals credentials with the sealing `secret-local` shares (`secret-seal`), under a
//! key that is a Workers secret binding: 64 hexadecimal digits, read once when the Worker
//! starts a request and never written anywhere. Reading a binding is `wasm32` only
//! ([`Worker::from_env`]); everything else, including the cases, runs natively.
//!
//! **Rotation.** A sealed value carries no key id, so a rotation needs both keys at once. The new
//! key is the binding `SEALING_KEY` and the old one is `SEALING_KEY_PREVIOUS`, read with
//! [`Worker::previous`]. Sealing always uses the current key. Opening tries the current key, then
//! the previous one, and says in its log which opened it. When nothing opens with the previous
//! key any more (every row was opened and sealed again), the binding is deleted.

mod error;
mod rotation;
mod worker;

pub use error::Error;
pub use rotation::Rotated;
pub use worker::Worker;
