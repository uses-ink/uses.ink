//! Plugins as wasm components, run by wasmtime (decisions 0007, 0015, 0035).
//!
//! A [`Runtime`] is one wasmtime engine with fuel and epochs on, the [`Budget`] of every call,
//! an optional [`Cache`] of compiled components and the host's capabilities (the pen through
//! `freehand`, text, highlighting through host-native's grammars, and the record of what a
//! plugin says), implemented once and linked into every component. It loads a format or a theme
//! as a [`Component`], which implements the engine's own [`engine::Format`] or [`engine::Theme`],
//! so the engine runs a component as it runs a native plugin.
//!
//! Every call gets a fresh budget. Running out of fuel, of time or of memory, a trap, or a value
//! the model refuses is a typed [`Failure`] that becomes a diagnostic, never a panic, and the
//! host serves the next call: a format call gets an instance of its own, and a theme's instance,
//! which renders one site, is replaced after a failed call.
//!
//! wasmtime is seen only here; the conversions between its generated types and the model are
//! `wit/mirror/`, shared with the guests' SDK.

mod bind;
mod budget;
mod cache;
mod capability;
mod component;
mod error;
mod mirror;
mod runtime;
mod source;

pub use budget::Budget;
pub use cache::{Cache, Origin};
pub use component::{Component, Format, Theme};
pub use error::{Error, Failure};
pub use mirror::{Malformed, Unmirrored};
pub use runtime::Runtime;
pub use source::{Source, Terms, Told, Unsourced};
