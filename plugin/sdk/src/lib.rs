//! The guest side of the plugin boundary (decision 0035): what a plugin built as a wasm component
//! links to reach its host and to speak the model.
//!
//! - [`bind`]: the bindings `wit-bindgen` writes for `ink:host/plugin`, every host capability and
//!   every model type in WIT. A slot's guest maps its world's imports onto these with `with`, so
//!   one set of types exists per component.
//! - The conversions between those types and the model, written once for both sides in
//!   `wit/mirror.rs` and included here, as `From` and `TryFrom` impls; [`Unmirrored`] and
//!   [`Malformed`] are their refusals.
//! - [`Host`]: the host's capabilities as the native traits a plugin's core is written against:
//!   [`Highlight`](model::highlight::Highlight) and [`Record`](model::Record).

pub mod bind;
mod host;
mod mirror;

pub use host::{Failure, Host};
pub use mirror::{Malformed, Unmirrored};
