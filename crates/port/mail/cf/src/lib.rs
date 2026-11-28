//! The mail port on Cloudflare (decision 0134).
//!
//! [`Cf`] turns a [`mail::Message`] into a [`Request`] (from, to, subject, plain-text body) and
//! hands it to a [`Transport`]. Which provider receives it (Email Service, a relay over `fetch`) is
//! open (`docs/hosted.md` HQ9), so the call is not made here: a transport is the one place that
//! knows an endpoint and a credential, and [`Fake`] is the transport the suite and the rig use.
//! Nothing in this crate needs `wasm32`.

mod cf;
mod error;
mod fake;
mod request;
mod transport;

pub use cf::Cf;
pub use error::Error;
pub use fake::{Down, Fake};
pub use request::Request;
pub use transport::Transport;
